//! TTPF texture packs (`RDS5` container, `ETPF` table).
//!
//! The container is a virtual byte stream split into chunks, each stored raw,
//! zlib, or in the console's hardware BCPack codec (which Vice Lens cannot
//! decode). The `ETPF` table describes each texture and is followed by a
//! sorted table of 64-bit keys that materials and UI use to refer to them.

use std::path::Path;

use crate::archive::{Archive, inflate};
use crate::swizzle::{self, Tile};
use crate::{Result, bad, le};

#[derive(Clone, Debug)]
pub struct Entry {
    pub key: u64,
    pub width: u32,
    pub height: u32,
    pub dxgi: u8,
    pub mips: bool,
    /// Start of the allocation in the virtual stream.
    pub offset: u64,
    /// Allocation size (to the next texture).
    pub alloc: u64,
    /// The record as stored, for research.
    pub raw: [u32; 4],
}

#[derive(Clone, Copy, Debug)]
struct Chunk {
    file: u64,
    file_end: u64,
    virt: u64,
    virt_len: u64,
    codec: u8,
}

pub struct Pack {
    archive: Archive,
    pub entries: Vec<Entry>,
    chunks: Vec<Chunk>,
}

impl Pack {
    pub fn open(path: impl AsRef<Path>) -> Result<Pack> {
        let archive = Archive::open(path)?;
        let b = archive.bytes();
        if !b.starts_with(b"RDS5") {
            return bad("not RDS5");
        }
        let n_chunks = le::u32(b, 4) as usize;
        let etpf = le::u32(b, 16) as usize * 0x100;
        if b.get(etpf..etpf + 4) != Some(b"ETPF") {
            return bad("no ETPF table");
        }
        let count = le::u32(b, etpf + 8) as usize;
        let descs = etpf + 16;
        let keys = descs + count * 16;
        if keys + count * 8 > b.len() {
            return bad("ETPF table out of range");
        }
        let mut entries: Vec<Entry> = (0..count)
            .map(|i| {
                let d = descs + i * 16;
                let (u0, u1, u2) = (le::u32(b, d), le::u32(b, d + 4), le::u32(b, d + 8));
                Entry {
                    key: le::u64(b, keys + i * 8),
                    width: (u0 & 0x3fff) + 1,
                    height: ((u0 >> 14) & 0x3fff) + 1,
                    dxgi: ((u1 >> 3) & 0x7f) as u8,
                    mips: u1 >> 24 != 0,
                    // 21 bits of 4 KiB pages: the largest packs pass 4 GiB
                    // (a 20-bit read put 21 601 textures 4 GiB too low).
                    offset: (u2 & 0x1f_ffff) as u64 * 0x1000,
                    alloc: 0,
                    raw: [u0, u1, u2, le::u32(b, d + 12)],
                }
            })
            .collect();
        // Chunk table right after the 24-byte header: per chunk, its end in
        // the file (256-byte units), the codec of the next chunk, and its end
        // in the virtual stream (16 KiB units plus 0..3 extra 4 KiB pages).
        let mut chunks = Vec::with_capacity(n_chunks);
        let (mut file, mut virt, mut codec) = (etpf as u64, 0u64, 0u8);
        for i in 0..n_chunks {
            let lo = le::u32(b, 24 + i * 8);
            let hi = le::u32(b, 24 + i * 8 + 4);
            let file_end = (lo & 0x03ff_ffff) as u64 * 0x100;
            let virt_end = (hi & 0xf_ffff) as u64 * 0x4000 + (lo >> 30) as u64 * 0x1000;
            if file_end > file && virt_end > virt {
                chunks.push(Chunk { file, file_end, virt, virt_len: virt_end - virt, codec });
                virt = virt_end;
            }
            file = file_end;
            codec = ((lo >> 26) & 0xf) as u8;
        }
        // Each texture runs to the next one's start, the last to the end of
        // the stream (the header's +8 holds only its low 32 bits).
        let total = virt;
        let mut starts: Vec<u64> = entries.iter().map(|e| e.offset).collect();
        starts.sort_unstable();
        starts.dedup();
        for e in &mut entries {
            let i = starts.partition_point(|&s| s <= e.offset);
            e.alloc = starts.get(i).copied().unwrap_or(total).saturating_sub(e.offset);
        }
        Ok(Pack { archive, entries, chunks })
    }

    pub fn find(&self, key: u64) -> Option<usize> {
        self.entries.binary_search_by_key(&key, |e| e.key).ok()
    }

    /// The chunks over virtual bytes `from..to`: (codec, start, end).
    pub fn chunks_over(&self, from: u64, to: u64) -> Vec<(u8, u64, u64)> {
        let first = self.chunks.partition_point(|c| c.virt + c.virt_len <= from);
        self.chunks[first..].iter().take_while(|c| c.virt < to).map(|c| (c.codec, c.virt, c.virt + c.virt_len)).collect()
    }

    /// Whether entry `i` is stored only in codecs Vice Lens reads (raw, zlib),
    /// so all of its pixels can be decoded.
    pub fn plain(&self, i: usize) -> bool {
        let e = &self.entries[i];
        self.chunks_over(e.offset, e.offset + e.alloc.max(1)).iter().all(|c| c.0 == 0 || c.0 == 8)
    }

    /// Whether the top mip of entry `i` is stored in codecs Vice Lens reads.
    /// Many textures keep only their small mips (stored first) in BCPack
    /// and the top level in zlib, so the top level decodes.
    pub fn top_plain(&self, i: usize) -> bool {
        self.top_span(i).is_ok_and(|(at, size, _)| self.chunks_over(at, at + size).iter().all(|c| c.0 == 0 || c.0 == 8))
    }

    /// Where the top mip of entry `i` lies in the virtual stream, its size
    /// and its tile mode.
    fn top_span(&self, i: usize) -> Result<(u64, u64, Tile)> {
        let e = &self.entries[i];
        let f = swizzle::format(e.dxgi).ok_or_else(|| crate::Error::Format(format!("dxgi {}", e.dxgi)))?;
        let linear = f.linear_size(e.width, e.height);
        // Mipmapped textures use 4 KiB tiles; others switch to 64 KiB tiles
        // once larger than one 64 KiB tile.
        let tile = if (!e.mips || !f.block) && linear > 65536 { Tile::K64 } else { Tile::K4 };
        let size = swizzle::tiled_size(f, e.width, e.height, tile) as u64;
        // Mips are stored smallest first: the top level ends the allocation,
        // unless the allocation is larger than the texture can fill. Each
        // level takes whole 4 KiB tiles, so a small texture's chain is much
        // more than 4/3 of its top level (1.7 times for 256×256 BC7, 2.7
        // for 512×64 BC1); the whole chain so padded bounds it.
        let chain: u64 = if e.mips {
            let levels = 32 - e.width.max(e.height).leading_zeros();
            (0..levels).map(|l| swizzle::tiled_size(f, (e.width >> l).max(1), (e.height >> l).max(1), Tile::K4) as u64).sum()
        } else {
            0
        };
        let fits = (e.alloc as f64) <= size as f64 * 1.65 || e.alloc <= chain;
        let at = if e.alloc > size && fits { e.offset + e.alloc - size } else { e.offset };
        Ok((at, size, tile))
    }

    /// Where `top_mip` reads entry `i`'s top level: the start of its
    /// allocation (true) or its end (false). For surveys.
    pub fn top_at_start(&self, i: usize) -> bool {
        self.top_span(i).is_ok_and(|(at, _, _)| at == self.entries[i].offset)
    }

    /// Read `len` bytes of the virtual stream at `off`.
    pub fn read_virtual(&self, off: u64, len: usize) -> Result<Vec<u8>> {
        let end = off + len as u64;
        let mut out = vec![0u8; len];
        let first = self.chunks.partition_point(|c| c.virt + c.virt_len <= off);
        let b = self.archive.bytes();
        for c in &self.chunks[first..] {
            if c.virt >= end {
                break;
            }
            let raw = &b[c.file as usize..(c.file_end as usize).min(b.len())];
            let data = match c.codec {
                8 => inflate(raw, c.virt_len as usize + 1).map(|r| r.0)?,
                0 => raw.to_vec(),
                k => return bad(format!("texture data uses the console's BCPack codec ({k})")),
            };
            let s0 = c.virt.max(off);
            let s1 = (c.virt + data.len() as u64).min(end);
            if s1 > s0 {
                out[(s0 - off) as usize..(s1 - off) as usize].copy_from_slice(&data[(s0 - c.virt) as usize..(s1 - c.virt) as usize]);
            }
        }
        Ok(out)
    }

    /// Mip `level` of entry `i` (0 is the top), linear, with its size.
    /// Mips are stored smallest first, each in whole 4 KiB tiles, so level
    /// `n` ends where level `n - 1` begins.
    pub fn mip(&self, i: usize, level: u32) -> Result<(u32, u32, Vec<u8>)> {
        let e = &self.entries[i];
        if level == 0 || !e.mips {
            let d = self.top_mip(i)?;
            return Ok((e.width, e.height, d));
        }
        let f = swizzle::format(e.dxgi).ok_or_else(|| crate::Error::Format(format!("dxgi {}", e.dxgi)))?;
        let (at, size, w, h) = self.mip_span(i, level)?;
        let raw = self.read_virtual(at, size as usize)?;
        Ok((w, h, swizzle::untile(&raw, f, w, h, Tile::K4)))
    }

    /// Where mip `level` (at least 1) of a mipmapped entry lies: start,
    /// bytes, width, height.
    fn mip_span(&self, i: usize, level: u32) -> Result<(u64, u64, u32, u32)> {
        let e = &self.entries[i];
        let f = swizzle::format(e.dxgi).ok_or_else(|| crate::Error::Format(format!("dxgi {}", e.dxgi)))?;
        let mut end = e.offset + e.alloc;
        let (mut w, mut h) = (e.width, e.height);
        for _ in 0..level {
            end = end.saturating_sub(swizzle::tiled_size(f, w, h, Tile::K4) as u64);
            w = (w / 2).max(1);
            h = (h / 2).max(1);
        }
        let size = swizzle::tiled_size(f, w, h, Tile::K4) as u64;
        if end < e.offset || size > end - e.offset {
            return bad("mip outside the allocation");
        }
        Ok((end - size, size, w, h))
    }

    /// Top mip read with a given tile mode, ending the allocation (research).
    pub fn top_mip_tiled(&self, i: usize, tile: Tile) -> Result<Vec<u8>> {
        let e = &self.entries[i];
        let f = swizzle::format(e.dxgi).ok_or_else(|| crate::Error::Format(format!("dxgi {}", e.dxgi)))?;
        let size = swizzle::tiled_size(f, e.width, e.height, tile) as u64;
        let at = (e.offset + e.alloc).saturating_sub(size).max(e.offset);
        let raw = self.read_virtual(at, size as usize)?;
        Ok(swizzle::untile(&raw, f, e.width, e.height, tile))
    }

    /// The linear top mip of entry `i`.
    pub fn top_mip(&self, i: usize) -> Result<Vec<u8>> {
        let e = &self.entries[i];
        let f = swizzle::format(e.dxgi).ok_or_else(|| crate::Error::Format(format!("dxgi {}", e.dxgi)))?;
        let (at, size, tile) = self.top_span(i)?;
        let raw = self.read_virtual(at, size as usize)?;
        Ok(swizzle::untile(&raw, f, e.width, e.height, tile))
    }
}

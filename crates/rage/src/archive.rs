//! Read-only access to the game's archive files and the zlib streams in them.
//!
//! RPF9 directories are encrypted, so Vice Lens does not list files: it finds
//! zlib streams (16-byte aligned) and recognises what they contain.

use std::fs::File;
use std::path::{Path, PathBuf};

use flate2::{Decompress, FlushDecompress, Status};
use memmap2::Mmap;

use crate::{Result, bad};

pub struct Archive {
    path: PathBuf,
    map: Mmap,
}

/// One zlib stream inside an archive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stream {
    /// Byte offset of the zlib header in the archive.
    pub offset: u64,
    /// Compressed length, header and checksum included.
    pub packed: u32,
    /// Decompressed length.
    pub size: u32,
}

impl Archive {
    pub fn open(path: impl AsRef<Path>) -> Result<Archive> {
        let path = path.as_ref().to_path_buf();
        let file = File::open(&path)?;
        // Safety: the game files are not modified while Vice Lens runs; the map
        // is read-only.
        let map = unsafe { Mmap::map(&file)? };
        Ok(Archive { path, map })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn bytes(&self) -> &[u8] {
        &self.map
    }

    pub fn len(&self) -> u64 {
        self.map.len() as u64
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Inflate the stream starting at `offset`; `limit` caps the output.
    pub fn inflate_at(&self, offset: u64, limit: usize) -> Result<(Vec<u8>, Stream)> {
        let data = self.map.get(offset as usize..).unwrap_or(&[]);
        let (out, packed) = inflate(data, limit)?;
        let stream = Stream { offset, packed: packed as u32, size: out.len() as u32 };
        Ok((out, stream))
    }

    pub fn read(&self, s: &Stream) -> Result<Vec<u8>> {
        let (out, _) = self.inflate_at(s.offset, s.size as usize + 1)?;
        if out.len() != s.size as usize {
            return bad(format!("stream at {:#x} changed size", s.offset));
        }
        Ok(out)
    }

    /// Every zlib stream in the archive, in file order. `visit` receives the
    /// stream and its decompressed bytes; returning `false` stops the scan.
    pub fn scan(&self, visit: impl FnMut(Stream, &[u8]) -> bool) {
        self.scan_from(0, visit)
    }

    /// Like `scan`, starting at `start` (rounded down to 16 bytes).
    pub fn scan_from(&self, start: u64, mut visit: impl FnMut(Stream, &[u8]) -> bool) {
        let data: &[u8] = &self.map;
        let mut pos = (start as usize) & !15;
        let mut out = Vec::new();
        while pos + 2 <= data.len() {
            if is_zlib_header(data[pos], data[pos + 1]) {
                out.clear();
                if let Ok(packed) = inflate_into(&data[pos..], &mut out, 1 << 30)
                    && !out.is_empty()
                {
                    let s = Stream { offset: pos as u64, packed: packed as u32, size: out.len() as u32 };
                    if !visit(s, &out) {
                        return;
                    }
                    pos = (pos + packed + 15) & !15;
                    continue;
                }
            }
            pos += 16;
        }
    }
}

/// Files are stored as 64 KiB pieces, each its own zlib stream, placed back
/// to back (the next piece starts at the next 16-byte boundary). A piece
/// shorter than 64 KiB ends its file.
pub const PIECE: u32 = 1 << 16;

/// A file: the run of streams it was stored as.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Piece {
    pub streams: Vec<Stream>,
}

impl Piece {
    pub fn offset(&self) -> u64 {
        self.streams[0].offset
    }
    pub fn size(&self) -> usize {
        self.streams.iter().map(|s| s.size as usize).sum()
    }
}

impl Archive {
    /// The file whose first stream starts at `offset`.
    pub fn file_at(&self, offset: u64) -> Result<Vec<u8>> {
        let mut streams = Vec::new();
        self.scan_from(offset, |s, _| {
            streams.push(s);
            s.size == PIECE && streams.len() < 4096
        });
        match streams.first() {
            Some(s) if s.offset == offset => self.read_file(&Archive::files(&streams)[0]),
            _ => bad(format!("no stream at {offset:#x}")),
        }
    }

    /// The resource whose first stream starts at `offset`.
    pub fn resource_at(&self, offset: u64) -> Result<crate::rsc::Resource> {
        let mut streams = Vec::new();
        self.scan_from(offset, |s, _| {
            streams.push(s);
            streams.len() < 128
        });
        let first = *streams.first().ok_or_else(|| crate::Error::Format("no stream".into()))?;
        if first.offset != offset {
            return bad(format!("no stream at {offset:#x}"));
        }
        let next = |c: u64| {
            let at = (c + 15) & !15;
            streams.iter().find(|s| s.offset >= at).copied()
        };
        crate::rsc::read(self, first, next)
    }

    /// Group streams (in file order) into files.
    pub fn files(streams: &[Stream]) -> Vec<Piece> {
        let mut out: Vec<Piece> = Vec::new();
        let mut continues = false;
        for s in streams {
            if continues {
                let last = *out.last().unwrap().streams.last().unwrap();
                let end = (last.offset + last.packed as u64 + 15) & !15;
                if s.offset == end {
                    out.last_mut().unwrap().streams.push(*s);
                    continues = s.size == PIECE;
                    continue;
                }
            }
            out.push(Piece { streams: vec![*s] });
            continues = s.size == PIECE;
        }
        out
    }

    /// Decompress a whole file.
    pub fn read_file(&self, f: &Piece) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(f.size());
        for s in &f.streams {
            out.extend_from_slice(&self.read(s)?);
        }
        Ok(out)
    }
}

fn is_zlib_header(cmf: u8, flg: u8) -> bool {
    cmf == 0x78 && (u16::from(cmf) << 8 | u16::from(flg)) % 31 == 0 && flg & 0x20 == 0
}

/// Inflate one zlib stream; returns the output and the compressed length.
pub fn inflate(data: &[u8], limit: usize) -> Result<(Vec<u8>, usize)> {
    let mut out = Vec::new();
    let packed = inflate_into(data, &mut out, limit)?;
    Ok((out, packed))
}

fn inflate_into(data: &[u8], out: &mut Vec<u8>, limit: usize) -> Result<usize> {
    let mut d = Decompress::new(true);
    let mut chunk = vec![0u8; 1 << 16];
    loop {
        let before_in = d.total_in() as usize;
        let before_out = d.total_out();
        let status = d.decompress(&data[before_in..], &mut chunk, FlushDecompress::None).map_err(|e| crate::Error::Format(format!("zlib: {e}")))?;
        let produced = (d.total_out() - before_out) as usize;
        out.extend_from_slice(&chunk[..produced]);
        if out.len() > limit {
            return bad("stream exceeds limit");
        }
        match status {
            Status::StreamEnd => return Ok(d.total_in() as usize),
            _ if produced == 0 && d.total_in() as usize == before_in => {
                return bad("truncated zlib stream");
            }
            _ => {}
        }
    }
}

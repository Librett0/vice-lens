//! Texture dictionaries (`pgDictionary<grcTexture>`).

use crate::rsc::{pptr, vptr};
use crate::{le, swizzle};

/// One texture's description from a dictionary.
#[derive(Clone, Debug)]
pub struct TextureInfo {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub dxgi: u8,
    pub mips: u8,
    /// Whether the pixels are tiled (`SW_4KB_S`).
    pub tiled: bool,
    /// Offset of the pixel allocation in the physical segment, if embedded.
    pub data: Option<usize>,
    /// The texture struct's offset in the virtual segment (for research).
    pub at: usize,
}

/// Dictionary header: `+0x20` hash array, `+0x28` count; `+0x30` item array,
/// `+0x38` count, `+0x3a` capacity.
pub fn probe(v: &[u8]) -> Option<usize> {
    let hashes = vptr(le::u64(v, 0x20))?;
    let items = vptr(le::u64(v, 0x30))?;
    let n = le::u16(v, 0x38) as usize;
    let (hash_count, capacity) = (le::u16(v, 0x28) as usize, le::u16(v, 0x3a) as usize);
    if n == 0 || n > 4096 || hash_count != n || capacity < n {
        return None;
    }
    if hashes + n * 4 > v.len() || items + n * 8 > v.len() {
        return None;
    }
    let first = vptr(le::u64(v, items))?;
    let (w, h) = (le::u16(v, first + 0x38), le::u16(v, first + 0x3a));
    (w > 0 && h > 0 && swizzle::format(le::u8(v, first + 0x3f)).is_some()).then_some(n)
}

pub fn textures(v: &[u8]) -> Vec<TextureInfo> {
    let Some(n) = probe(v) else { return Vec::new() };
    let items = vptr(le::u64(v, 0x30)).unwrap();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let Some(t) = vptr(le::u64(v, items + i * 8)) else { continue };
        let name = vptr(le::u64(v, t + 0x30))
            .map(|mut o| {
                // Some names are preceded by 8 zero bytes.
                if le::u64(v, o) == 0 {
                    o += 8;
                }
                String::from_utf8_lossy(le::cstr(v, o)).into_owned()
            })
            .unwrap_or_default();
        out.push(TextureInfo {
            name,
            width: le::u16(v, t + 0x38) as u32,
            height: le::u16(v, t + 0x3a) as u32,
            dxgi: le::u8(v, t + 0x3f),
            tiled: le::u8(v, t + 0x40) != 0,
            mips: le::u8(v, t + 0x42),
            data: pptr(le::u64(v, t + 0x18)),
            at: t,
        });
    }
    out
}

/// Linear (untiled) top mip of an embedded texture. Mips are stored smallest
/// first, so the top level sits at the end of the allocation; `alloc` is the
/// allocation's size.
pub fn top_mip(phys: &[u8], t: &TextureInfo, alloc: usize) -> Option<Vec<u8>> {
    let fmt = swizzle::format(t.dxgi)?;
    let start = t.data?;
    if !t.tiled {
        let need = fmt.linear_size(t.width, t.height);
        let off = if t.mips > 1 && alloc >= need { start + alloc - need } else { start };
        return phys.get(off..off + need).map(<[u8]>::to_vec);
    }
    let need = swizzle::tiled_size(fmt, t.width, t.height, swizzle::Tile::K4);
    let off = if t.mips > 1 && alloc >= need { start + alloc - need } else { start };
    let src = phys.get(off..(off + need).min(phys.len()))?;
    Some(swizzle::untile(src, fmt, t.width, t.height, swizzle::Tile::K4))
}

/// Allocation sizes: the distance from each texture's data to the next one.
pub fn allocations(texs: &[TextureInfo], phys_len: usize) -> Vec<usize> {
    let mut starts: Vec<usize> = texs.iter().filter_map(|t| t.data).collect();
    starts.sort_unstable();
    starts.dedup();
    texs.iter()
        .map(|t| {
            let Some(s) = t.data else { return 0 };
            let i = starts.partition_point(|&x| x <= s);
            starts.get(i).copied().unwrap_or(phys_len) - s
        })
        .collect()
}

//! Navigation meshes (`CNavMesh`, version 0x00010015): the walkable and
//! drivable surface, in world coordinates, one 128 m tile per resource
//! (`nav_<gx>_<gy>`; the tile's world origin is ((g - 256) × 128)).
//!
//! Virtual segment: `+0x14` version, `+0x60` extent (3 f32), `+0xa0`
//! quadtree whose first 12 bytes are the minimum corner. Vertices (`+0x70`),
//! indices (`+0x80`) and polygons (`+0x98`) are split arrays: `+0x10`
//! pointer to blocks of (pointer, u32 count), `+0x20` u32 block count.
//! Vertices are 3 × u16 over the extent; indices u16; polygons 44 bytes:
//! `+4` (first index << 4 | vertex count), flags at `+8` and `+0x18`.

use crate::le;
use crate::rsc::vptr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Land,
    Water,
    Road,
    Pavement,
    Interior,
}

#[derive(Debug, Default)]
pub struct NavMesh {
    pub positions: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
    pub surfaces: Vec<Surface>,
}

fn f3(v: &[u8], o: usize) -> [f32; 3] {
    [0, 4, 8].map(|i| f32::from_bits(le::u32(v, o + i)))
}

fn split(v: &[u8], at: usize) -> Option<Vec<(usize, usize)>> {
    let s = vptr(le::u64(v, at))?;
    let blocks = vptr(le::u64(v, s + 0x10))?;
    let n = le::u32(v, s + 0x20) as usize;
    if n == 0 || n > 4096 || blocks + n * 16 > v.len() {
        return None;
    }
    (0..n).map(|i| Some((vptr(le::u64(v, blocks + i * 16))?, le::u32(v, blocks + i * 16 + 8) as usize))).collect()
}

pub fn is_navmesh(v: &[u8]) -> bool {
    v.len() >= 0xd0 && le::u32(v, 0x14) == 0x0001_0015
}

pub fn parse(v: &[u8]) -> Option<NavMesh> {
    if !is_navmesh(v) {
        return None;
    }
    let ext = f3(v, 0x60);
    let min = f3(v, vptr(le::u64(v, 0xa0))?);
    let mut m = NavMesh::default();
    for (b, n) in split(v, 0x70)? {
        for i in 0..n {
            let o = b + i * 6;
            if o + 6 > v.len() {
                break;
            }
            let q = |k: usize| le::u16(v, o + k * 2) as f32 / 65535.0 * ext[k] + min[k];
            m.positions.push([q(0), q(1), q(2)]);
        }
    }
    let mut idx = Vec::new();
    for (b, n) in split(v, 0x80)? {
        for i in 0..n {
            idx.push(le::u16(v, b + i * 2) as u32);
        }
    }
    for (b, n) in split(v, 0x98)? {
        for i in 0..n {
            let o = b + i * 44;
            if o + 44 > v.len() {
                break;
            }
            let info = le::u32(v, o + 4);
            let (count, first) = ((info & 0xf) as usize, (info >> 4) as usize);
            if count < 3 || first + count > idx.len() {
                continue;
            }
            let (f8, f18) = (le::u32(v, o + 8), le::u32(v, o + 0x18));
            let s = if f8 & 0x80 != 0 || f18 & 0x8_0000 != 0 {
                Surface::Water
            } else if f8 & 0x08 != 0 {
                Surface::Interior
            } else if f18 & 0x2000 != 0 {
                Surface::Road
            } else if f18 & 0x1000 != 0 {
                Surface::Pavement
            } else {
                Surface::Land
            };
            for k in 1..count - 1 {
                let t = [idx[first], idx[first + k], idx[first + k + 1]];
                if t.iter().all(|&i| (i as usize) < m.positions.len()) {
                    m.triangles.push(t);
                    m.surfaces.push(s);
                }
            }
        }
    }
    Some(m)
}

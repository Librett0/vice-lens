//! Pixel formats and the RDNA2 (GFX10) tiling the console stores textures in.
//!
//! Tiled textures use AMD's 4 KiB standard swizzle (`SW_4KB_S`: the texture
//! struct's byte at +0x40 is 5 for every texture seen). Inside a tile, each
//! bit of an element's index comes from one bit of its x or y coordinate;
//! the tables below list those bits from the lowest. The 4 KiB rows were
//! measured on the game's textures (`swzsearch`): the order that makes a
//! picture smoothest; the 64 KiB rows are not used.

/// A DXGI format as far as layout goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Format {
    pub dxgi: u8,
    /// Bytes per element (a pixel, or a 4×4 block for BC formats).
    pub bytes: usize,
    pub block: bool,
    pub srgb: bool,
}

impl Format {
    pub fn elements(&self, w: u32, h: u32) -> (usize, usize) {
        if self.block { (w.div_ceil(4).max(1) as usize, h.div_ceil(4).max(1) as usize) } else { (w.max(1) as usize, h.max(1) as usize) }
    }
    pub fn linear_size(&self, w: u32, h: u32) -> usize {
        let (ew, eh) = self.elements(w, h);
        ew * eh * self.bytes
    }
}

pub fn format(dxgi: u8) -> Option<Format> {
    let (bytes, block, srgb) = match dxgi {
        10 | 11 => (8, false, false),
        24 | 28 | 34 | 41 | 87 => (4, false, false),
        29 | 91 => (4, false, true),
        49 | 54 | 56 => (2, false, false),
        61 | 65 => (1, false, false),
        71 | 80 | 81 => (8, true, false),
        72 => (8, true, true),
        74 | 77 | 83 | 84 | 95 | 96 | 98 => (16, true, false),
        75 | 78 | 99 => (16, true, true),
        _ => return None,
    };
    Some(Format { dxgi, bytes, block, srgb })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tile {
    K4,
    K64,
}

#[derive(Clone, Copy)]
enum B {
    X(u8),
    Y(u8),
}
use B::{X, Y};

/// Tile size in elements and the index bit pattern, per element size.
fn pattern(bytes: usize, tile: Tile) -> (usize, usize, &'static [B]) {
    match (tile, bytes) {
        (Tile::K4, 16) => (16, 16, &[Y(0), Y(1), X(0), X(1), Y(2), X(2), Y(3), X(3)]),
        (Tile::K4, 8) => (32, 16, &[X(0), Y(0), Y(1), X(1), X(2), Y(2), X(3), Y(3), X(4)]),
        (Tile::K4, 4) => (32, 32, &[X(0), X(1), Y(0), Y(1), Y(2), X(2), Y(3), X(3), Y(4), X(4)]),
        (Tile::K4, 2) => (64, 32, &[X(0), X(1), X(2), Y(0), Y(1), Y(2), X(3), Y(3), X(4), Y(4), X(5)]),
        (Tile::K4, _) => (64, 64, &[X(0), X(1), X(2), X(3), Y(0), Y(1), Y(2), Y(3), Y(4), X(4), Y(5), X(5)]),
        (Tile::K64, 16) => (64, 64, &[X(0), Y(0), X(1), Y(1), Y(2), X(2), Y(3), X(3), Y(4), X(4), Y(5), X(5)]),
        (Tile::K64, 8) => (128, 64, &[X(0), Y(0), X(1), X(2), Y(1), Y(2), X(3), Y(3), X(4), Y(4), X(5), Y(5), X(6)]),
        (Tile::K64, 4) => (128, 128, &[X(0), X(1), Y(0), Y(1), X(2), Y(2), Y(3), X(3), Y(4), X(4), Y(5), X(5), Y(6), X(6)]),
        (Tile::K64, 2) => (256, 128, &[X(0), X(1), X(2), Y(0), Y(1), Y(2), X(3), Y(3), X(4), Y(4), X(5), Y(5), X(6), Y(6), X(7)]),
        (Tile::K64, _) => (256, 256, &[Y(0), Y(1), Y(2), X(1), X(0), X(2), Y(3), X(3), X(4), Y(4), X(5), Y(5), X(6), Y(6), X(7), Y(7)]),
    }
}

/// Bytes a tiled top mip occupies (whole tiles).
pub fn tiled_size(f: Format, w: u32, h: u32, tile: Tile) -> usize {
    let (ew, eh) = f.elements(w, h);
    let (tw, th, _) = pattern(f.bytes, tile);
    ew.div_ceil(tw) * tw * eh.div_ceil(th) * th * f.bytes
}

/// Untile a top mip into a linear, row-major element array. Missing source
/// bytes read as zero.
pub fn untile(src: &[u8], f: Format, w: u32, h: u32, tile: Tile) -> Vec<u8> {
    let (ew, eh) = f.elements(w, h);
    let (tw, th, bits) = pattern(f.bytes, tile);
    let tiles_x = ew.div_ceil(tw);
    let per_tile = tw * th;
    // Precompute the in-tile index contribution of each x and y.
    let part = |coord: usize, want_y: bool| -> usize {
        bits.iter().enumerate().fold(0, |acc, (i, b)| match (b, want_y) {
            (X(n), false) if coord >> n & 1 == 1 => acc | 1 << i,
            (Y(n), true) if coord >> n & 1 == 1 => acc | 1 << i,
            _ => acc,
        })
    };
    let xs: Vec<usize> = (0..tw).map(|x| part(x, false)).collect();
    let ys: Vec<usize> = (0..th).map(|y| part(y, true)).collect();
    let n = f.bytes;
    let mut out = vec![0u8; ew * eh * n];
    for y in 0..eh {
        let row_tile = (y / th) * tiles_x;
        let iy = ys[y % th];
        for x in 0..ew {
            let t = row_tile + x / tw;
            let s = (t * per_tile + (iy | xs[x % tw])) * n;
            let d = (y * ew + x) * n;
            if let Some(px) = src.get(s..s + n) {
                out[d..d + n].copy_from_slice(px);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every element of a tile maps to a distinct index (the pattern is a
    /// permutation) for each size and tile mode.
    #[test]
    fn patterns_are_permutations() {
        for tile in [Tile::K4, Tile::K64] {
            for bytes in [1, 2, 4, 8, 16] {
                let (tw, th, bits) = pattern(bytes, tile);
                assert_eq!(1 << bits.len(), tw * th);
                assert_eq!(tw * th * bytes, if tile == Tile::K4 { 4096 } else { 65536 });
                let mut seen = vec![false; tw * th];
                for y in 0..th {
                    for x in 0..tw {
                        let i = bits.iter().enumerate().fold(0usize, |acc, (k, b)| match b {
                            X(n) => acc | (x >> n & 1) << k,
                            Y(n) => acc | (y >> n & 1) << k,
                        });
                        assert!(!seen[i]);
                        seen[i] = true;
                    }
                }
            }
        }
    }
}

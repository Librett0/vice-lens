//! Scaleform GFX movies: the tag list, and fonts (`DefineFont3`).

use std::collections::HashMap;

use crate::{Result, bad, le};

#[derive(Clone, Copy, Debug)]
pub struct Tag<'a> {
    pub code: u16,
    pub body: &'a [u8],
}

pub fn is_gfx(b: &[u8]) -> bool {
    b.starts_with(b"GFX")
}

/// Tags of an uncompressed GFX file (SWF layout).
pub fn tags(b: &[u8]) -> Result<Vec<Tag<'_>>> {
    if !is_gfx(b) {
        return bad("not GFX");
    }
    let nbits = (le::u8(b, 8) >> 3) as usize;
    let mut p = 8 + (5 + 4 * nbits).div_ceil(8) + 4;
    let mut out = Vec::new();
    while p + 2 <= b.len() {
        let h = le::u16(b, p);
        p += 2;
        let code = h >> 6;
        let mut len = (h & 0x3f) as usize;
        if len == 0x3f {
            len = le::u32(b, p) as usize;
            p += 4;
        }
        let body = b.get(p..p + len).ok_or_else(|| crate::Error::Format("GFX tag past end".into()))?;
        out.push(Tag { code, body });
        p += len;
        if code == 0 {
            break;
        }
    }
    Ok(out)
}

/// A point in font units (DefineFont3: 20480 per em).
pub type Pt = [f32; 2];

#[derive(Clone, Debug)]
pub enum Seg {
    Move(Pt),
    Line(Pt),
    Quad(Pt, Pt),
}

#[derive(Clone, Debug, Default)]
pub struct Glyph {
    pub code: char,
    pub advance: f32,
    pub path: Vec<Seg>,
}

#[derive(Clone, Debug, Default)]
pub struct Font {
    pub name: String,
    pub bold: bool,
    pub italic: bool,
    pub ascent: f32,
    pub descent: f32,
    pub leading: f32,
    pub glyphs: Vec<Glyph>,
    pub by_char: HashMap<char, usize>,
    pub kerning: HashMap<(char, char), f32>,
}

/// Units per em in DefineFont3 glyph coordinates.
pub const EM: f32 = 20480.0;

impl Font {
    pub fn glyph(&self, c: char) -> Option<&Glyph> {
        self.by_char.get(&c).map(|&i| &self.glyphs[i])
    }
}

/// All `DefineFont3` fonts in a GFX file.
pub fn fonts(b: &[u8]) -> Result<Vec<Font>> {
    Ok(tags(b)?.iter().filter(|t| t.code == 75).filter_map(|t| define_font3(t.body).ok()).collect())
}

fn define_font3(b: &[u8]) -> Result<Font> {
    let flags = le::u8(b, 2);
    let has_layout = flags & 0x80 != 0;
    let wide_offsets = flags & 0x08 != 0;
    let name_len = le::u8(b, 4) as usize;
    let name_raw = b.get(5..5 + name_len).ok_or_else(|| crate::Error::Format("font name".into()))?;
    let name = String::from_utf8_lossy(le::cstr(name_raw, 0)).into_owned();
    let mut p = 5 + name_len;
    let n = le::u16(b, p) as usize;
    p += 2;
    let table = p;
    let off = |i: usize| if wide_offsets { le::u32(b, table + i * 4) as usize } else { le::u16(b, table + i * 2) as usize };
    let code_table = off(n);
    let mut glyphs = Vec::with_capacity(n);
    for i in 0..n {
        let start = table + off(i);
        let end = if i + 1 < n { table + off(i + 1) } else { table + code_table };
        let shape = b.get(start..end.max(start)).unwrap_or(&[]);
        glyphs.push(Glyph { path: shape_path(shape), ..Glyph::default() });
    }
    let mut q = table + code_table;
    for g in &mut glyphs {
        g.code = char::from_u32(le::u16(b, q) as u32).unwrap_or('\u{fffd}');
        q += 2;
    }
    let mut font = Font { name, bold: flags & 1 != 0, italic: flags & 2 != 0, ..Font::default() };
    if has_layout {
        font.ascent = le::u16(b, q) as f32;
        font.descent = le::u16(b, q + 2) as f32;
        font.leading = le::i16(b, q + 4) as f32;
        q += 6;
        for g in &mut glyphs {
            g.advance = le::i16(b, q) as f32;
            q += 2;
        }
        // Bounds: one RECT per glyph, variable length.
        let mut bits = Bits::new(b, q);
        for _ in 0..n {
            let nb = bits.ub(5);
            for _ in 0..4 {
                bits.sb(nb);
            }
            bits.align();
        }
        q = bits.byte();
        let kerning = le::u16(b, q) as usize;
        q += 2;
        for _ in 0..kerning {
            let a = char::from_u32(le::u16(b, q) as u32).unwrap_or('\0');
            let c = char::from_u32(le::u16(b, q + 2) as u32).unwrap_or('\0');
            font.kerning.insert((a, c), le::i16(b, q + 4) as f32);
            q += 6;
        }
    }
    font.by_char = glyphs.iter().enumerate().map(|(i, g)| (g.code, i)).collect();
    font.glyphs = glyphs;
    Ok(font)
}

/// SHAPE records of one glyph into a path (y down, font units).
fn shape_path(b: &[u8]) -> Vec<Seg> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Bits::new(b, 0);
    let fill_bits = r.ub(4);
    let line_bits = r.ub(4);
    let (mut x, mut y) = (0i32, 0i32);
    let mut open = false;
    loop {
        if r.done() {
            break;
        }
        if r.ub(1) == 0 {
            let new_styles = r.ub(1);
            let line = r.ub(1);
            let fill1 = r.ub(1);
            let fill0 = r.ub(1);
            let moveto = r.ub(1);
            if new_styles | line | fill1 | fill0 | moveto == 0 {
                break;
            }
            if moveto == 1 {
                let nb = r.ub(5);
                x = r.sb(nb);
                y = r.sb(nb);
                out.push(Seg::Move([x as f32, y as f32]));
                open = true;
            }
            if fill0 == 1 {
                r.ub(fill_bits);
            }
            if fill1 == 1 {
                r.ub(fill_bits);
            }
            if line == 1 {
                r.ub(line_bits);
            }
            if new_styles == 1 {
                // Fonts do not define styles; stop rather than misread.
                break;
            }
        } else {
            if !open {
                out.push(Seg::Move([x as f32, y as f32]));
                open = true;
            }
            let straight = r.ub(1) == 1;
            let nb = r.ub(4) + 2;
            if straight {
                let (dx, dy) = if r.ub(1) == 1 {
                    (r.sb(nb), r.sb(nb))
                } else if r.ub(1) == 1 {
                    (0, r.sb(nb))
                } else {
                    (r.sb(nb), 0)
                };
                x += dx;
                y += dy;
                out.push(Seg::Line([x as f32, y as f32]));
            } else {
                let (cx, cy) = (x + r.sb(nb), y + r.sb(nb));
                x = cx + r.sb(nb);
                y = cy + r.sb(nb);
                out.push(Seg::Quad([cx as f32, cy as f32], [x as f32, y as f32]));
            }
        }
    }
    out
}

struct Bits<'a> {
    b: &'a [u8],
    bit: usize,
}

impl<'a> Bits<'a> {
    fn new(b: &'a [u8], byte: usize) -> Self {
        Bits { b, bit: byte * 8 }
    }
    fn done(&self) -> bool {
        self.bit >= self.b.len() * 8
    }
    fn ub(&mut self, n: u32) -> u32 {
        let mut v = 0u32;
        for _ in 0..n {
            let byte = self.b.get(self.bit / 8).copied().unwrap_or(0);
            v = v << 1 | u32::from(byte >> (7 - self.bit % 8) & 1);
            self.bit += 1;
        }
        v
    }
    fn sb(&mut self, n: u32) -> i32 {
        if n == 0 {
            return 0;
        }
        let v = self.ub(n);
        ((v << (32 - n)) as i32) >> (32 - n)
    }
    fn align(&mut self) {
        self.bit = self.bit.div_ceil(8) * 8;
    }
    fn byte(&self) -> usize {
        self.bit.div_ceil(8)
    }
}

//! Text tables (`0DRPz`): label hash → UTF-8 string.

use std::collections::HashMap;

use crate::{Result, bad, joaat, le};

const VBASE: u64 = 0x5000_0000;

pub fn is_text_table(b: &[u8]) -> bool {
    b.get(0x10..0x15) == Some(b"0DRPz")
}

/// A loaded text table: label hash to string.
#[derive(Default)]
pub struct TextTable {
    strings: HashMap<u32, String>,
}

impl TextTable {
    pub fn get(&self, label: &str) -> Option<&str> {
        self.by_hash(joaat(label))
    }
    pub fn by_hash(&self, hash: u32) -> Option<&str> {
        self.strings.get(&hash).map(String::as_str)
    }
    pub fn len(&self) -> usize {
        self.strings.len()
    }
    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }
    pub fn iter(&self) -> impl Iterator<Item = (u32, &str)> {
        self.strings.iter().map(|(k, v)| (*k, v.as_str()))
    }
    /// Merge another table; existing labels win.
    pub fn merge(&mut self, other: TextTable) {
        for (k, v) in other.strings {
            self.strings.entry(k).or_insert(v);
        }
    }
}

/// Parse a large label table: a Swiss-style hash table of `(hash, index)`,
/// an index array of `(page, offset)`, and string pages, all listed in the
/// block list the header points at (pointer at 0x38, count at 0x4c).
/// Index entries name a page by its position in that list plus one.
pub fn parse(b: &[u8]) -> Result<TextTable> {
    if !is_text_table(b) {
        return bad("not a 0DRPz table");
    }
    let blocks = blocks(b)?;
    // The hash table: groups of 16 control bytes + 16 slots of 16 bytes.
    let table = blocks.iter().find(|bl| bl.size % 272 == 0 && bl.size >= 272 * 16).ok_or_else(|| err("no hash table"))?;
    let mut slots = Vec::new();
    for g in 0..table.size / 272 {
        let base = table.off + g * 272;
        for j in 0..16 {
            if le::u8(b, base + j) & 0x80 != 0 {
                continue;
            }
            let s = base + 16 + j * 16;
            slots.push((le::u32(b, s), (le::u64(b, s + 8) >> 24) as usize));
        }
    }
    let count = slots.iter().map(|s| s.1 + 1).max().unwrap_or(0);
    let index = blocks.iter().find(|bl| bl.size == count * 16).ok_or_else(|| err("no index array"))?;
    let mut strings = HashMap::with_capacity(slots.len());
    for (hash, idx) in slots {
        let v = le::u64(b, index.off + idx * 16 + 8);
        let Some(page) = ((v & 0xffff) as usize).checked_sub(1).and_then(|i| blocks.get(i)) else { continue };
        let off = (v >> 20) as usize;
        if off < page.size {
            strings.insert(hash, String::from_utf8_lossy(le::cstr(b, page.off + off)).into_owned());
        }
    }
    Ok(TextTable { strings })
}

struct Block {
    off: usize,
    size: usize,
}

fn err(msg: &str) -> crate::Error {
    crate::Error::Format(msg.into())
}

/// Block records: `u32 tag, u32 size, u64 pointer`.
fn blocks(b: &[u8]) -> Result<Vec<Block>> {
    let list = le::u64(b, 0x38).checked_sub(VBASE).ok_or_else(|| err("bad block list"))? as usize;
    let count = le::u16(b, 0x4c) as usize;
    if count == 0 || count > 64 {
        return bad("bad block count");
    }
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let r = list + i * 16;
        let size = le::u32(b, r + 4) as usize;
        let off = le::u64(b, r + 8).checked_sub(VBASE).ok_or_else(|| err("bad block pointer"))? as usize;
        if off + size > b.len() {
            return bad("block outside the table");
        }
        out.push(Block { off, size });
    }
    Ok(out)
}

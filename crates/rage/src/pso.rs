//! PSO (`PSIN` + `LSCH`/`LENM`/`LSNS`): the game's binary data for parsable
//! structures, here as stored in the console build.
//!
//! `PSIN` holds the data. After its 8-byte chunk header: `u32`, `u32 region
//! size`, then 16-byte block records (`8 × 0xaa`, `u32 type hash`, `u32 size`)
//! ended by a `0x70707070` record. Block 1 is a region of that size right
//! after the records; blocks 2.. follow it, each 16-byte aligned, in record
//! order. A pointer is a `u64`: block number in bits 0..12, byte offset within
//! the block from bit 20. Names are joaat hashes; the schema chunks are not
//! needed to read the structures Vice Lens uses.

use crate::{Result, bad, le};

#[derive(Clone, Copy, Debug)]
pub struct Block {
    pub offset: usize,
    pub size: usize,
    pub type_hash: u32,
}

pub struct Pso {
    pub data: Vec<u8>,
    pub blocks: Vec<Block>,
}

impl Pso {
    pub fn parse(data: Vec<u8>) -> Result<Pso> {
        if !data.starts_with(b"PSIN") {
            return bad("not a PSO");
        }
        let psin = u32::from_be_bytes(data[4..8].try_into().unwrap()) as usize;
        let region = le::u32(&data, 12) as usize;
        let mut types = Vec::new();
        let mut o = 0x10;
        while data.get(o..o + 8) == Some(&[0xaa; 8]) {
            let (h, size) = (le::u32(&data, o + 8), le::u32(&data, o + 12) as usize);
            if h == 0x7070_7070 {
                break;
            }
            types.push((h, size));
            o += 16;
        }
        let mut blocks = vec![Block { offset: o + 16, size: region, type_hash: 0 }];
        let mut cur = o + 16 + region;
        for (type_hash, size) in types {
            cur = cur.div_ceil(16) * 16;
            blocks.push(Block { offset: cur, size, type_hash });
            cur += size;
        }
        if cur.div_ceil(16) * 16 != psin.div_ceil(16) * 16 || cur > data.len() {
            return bad(format!("PSO blocks end at {cur:#x}, PSIN at {psin:#x}"));
        }
        Ok(Pso { data, blocks })
    }

    /// Resolve a pointer to (byte offset, block number, block type).
    pub fn resolve(&self, p: u64) -> Option<(usize, usize, u32)> {
        let b = (p & 0xfff) as usize;
        if b == 0 || (p >> 12) & 0xff != 0 {
            return None;
        }
        let block = self.blocks.get(b - 1)?;
        let off = (p >> 20) as usize;
        (off < block.size).then_some((block.offset + off, b, block.type_hash))
    }

    pub fn block_of_type(&self, type_hash: u32) -> Option<Block> {
        self.blocks.iter().copied().find(|b| b.type_hash == type_hash)
    }

    pub fn u8(&self, o: usize) -> u8 {
        le::u8(&self.data, o)
    }
    pub fn u16(&self, o: usize) -> u16 {
        le::u16(&self.data, o)
    }
    pub fn u32(&self, o: usize) -> u32 {
        le::u32(&self.data, o)
    }
    pub fn u64(&self, o: usize) -> u64 {
        le::u64(&self.data, o)
    }
    pub fn f32(&self, o: usize) -> f32 {
        f32::from_bits(self.u32(o))
    }
}

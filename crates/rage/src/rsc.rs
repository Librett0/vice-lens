//! RSC9 resources: a virtual segment (pointers `0x5…`) and a physical one
//! (pointers `0x6…`), each stored as one or more consecutive zlib streams.

use crate::archive::{Archive, Stream};
use crate::{Result, bad, le};

pub const VIRTUAL: u64 = 0x5000_0000;
pub const PHYSICAL: u64 = 0x6000_0000;

pub fn vptr(p: u64) -> Option<usize> {
    (VIRTUAL..VIRTUAL + 0x0800_0000).contains(&p).then(|| (p - VIRTUAL) as usize)
}

pub fn pptr(p: u64) -> Option<usize> {
    (PHYSICAL..PHYSICAL + 0x1000_0000).contains(&p).then(|| (p - PHYSICAL) as usize)
}

/// A resource's first page looks like this: 8 zero bytes (no vtable on disk),
/// then the block map pointer.
pub fn looks_like_resource(first: &[u8]) -> bool {
    first.len() >= 0x40 && le::u64(first, 0) == 0 && vptr(le::u64(first, 8)).is_some() && !crate::text::is_text_table(first)
}

pub struct Resource {
    pub virt: Vec<u8>,
    pub phys: Vec<u8>,
    /// The streams it was read from, in order.
    pub streams: Vec<Stream>,
}

/// A block larger than this is stored as streams of this size and a last,
/// smaller one; only that last stream ends the block.
pub const SPLIT: u32 = 1 << 25;

/// Read a resource whose first stream is `first`: the block map (pointer at
/// +8) holds the number of virtual blocks at +8 and physical at +9; their
/// streams follow the first in file order.
pub fn read(archive: &Archive, first: Stream, next: impl Fn(u64) -> Option<Stream>) -> Result<Resource> {
    let mut virt = archive.read(&first)?;
    if virt.len() < 0x40 || le::u64(&virt, 0) != 0 || vptr(le::u64(&virt, 8)).is_none() {
        return bad("not a resource");
    }
    let mut streams = vec![first];
    let mut cursor = first.offset + first.packed as u64;
    let map = vptr(le::u64(&virt, 8)).unwrap();
    let ends_block = |s: &Stream| s.size != SPLIT;
    // The block map itself may sit in a later virtual stream.
    let mut v_count = None;
    let mut blocks = usize::from(ends_block(&first));
    loop {
        if v_count.is_none() && map + 10 <= virt.len() {
            v_count = Some((le::u8(&virt, map + 8) as usize).max(1));
        }
        if let Some(n) = v_count
            && blocks >= n
        {
            break;
        }
        if streams.len() > 64 {
            return bad("resource with too many virtual streams");
        }
        let s = next(cursor).ok_or_else(|| crate::Error::Format("missing virtual stream".into()))?;
        virt.extend_from_slice(&archive.read(&s)?);
        cursor = s.offset + s.packed as u64;
        blocks += usize::from(ends_block(&s));
        streams.push(s);
    }
    let p_count = le::u8(&virt, map + 9) as usize;
    let mut phys = Vec::new();
    let mut blocks = 0;
    while blocks < p_count {
        let s = next(cursor).ok_or_else(|| crate::Error::Format("missing physical stream".into()))?;
        phys.extend_from_slice(&archive.read(&s)?);
        cursor = s.offset + s.packed as u64;
        blocks += usize::from(ends_block(&s));
        streams.push(s);
        if streams.len() > 512 {
            return bad("resource with too many physical streams");
        }
    }
    Ok(Resource { virt, phys, streams })
}

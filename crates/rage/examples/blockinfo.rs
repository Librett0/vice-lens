//! An animation's blocks as stored: data length, frame data offset, frames,
//! bytes per frame, where the channel counts would lie, and the counts.
//!
//! `blockinfo <archive> <dictionary offset> <animation virt offset>...`
use rage::{Archive, anim, le};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let hex = |s: &str| u64::from_str_radix(s.trim_start_matches("0x"), 16).unwrap() as usize;
    let v = ar.resource_at(hex(&a[1]) as u64).unwrap().virt;
    println!("virtual segment {:#x} bytes", v.len());
    for x in &a[2..] {
        let an = anim::animation(&v, hex(x)).unwrap();
        println!("{} frames {} per block {} tracks {} blocks {}", an.name, an.frames, an.per_block, an.tracks.len(), an.blocks.len());
        for &b in &an.blocks {
            let len = le::u32(&v, b + 4) as usize;
            let (fo, nf, fl) = (le::u32(&v, b + 12) as usize, le::u16(&v, b + 0x16) as usize, le::u16(&v, b + 0x18) as usize);
            let at = b + 0x20 + fo + fl * nf;
            let head: Vec<String> = (0..8).map(|k| format!("{:x}", le::u32(&v, b + k * 4))).collect();
            let counts: Vec<u16> = if at + 22 <= v.len() { (0..11).map(|t| le::u16(&v, at + t * 2)).collect() } else { vec![] };
            println!("  block {b:#x}: len {len:#x} frame data {fo:#x} frames {nf} × {fl} → counts at +{:#x} {counts:?}; header {}", at - b, head.join(" "));
        }
    }
}

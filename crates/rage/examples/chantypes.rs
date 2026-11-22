//! Per animation of a clip dictionary: its blocks' channel counts by type
//! (0–10), to see which types a dictionary's motion uses.
//!
//! `chantypes <archive> <dictionary offset>`
use rage::{Archive, anim, le};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let v = ar.resource_at(u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap()).unwrap().virt;
    let (name, clips) = anim::dictionary(&v).unwrap();
    println!("{name}");
    for c in &clips {
        for p in &c.parts {
            let Some(an) = anim::animation(&v, p.animation) else { continue };
            for &b in &an.blocks {
                let len = le::u32(&v, b + 4) as usize;
                let (frame_off, frames, flen) = (le::u32(&v, b + 12) as usize, le::u16(&v, b + 0x16) as usize, le::u16(&v, b + 0x18) as usize);
                let d = &v[b + 0x20..(b + 0x20 + len).min(v.len())];
                let list = frame_off + flen * frames;
                let counts: Vec<u16> = (0..11).map(|t| le::u16(d, list + t * 2)).collect();
                println!("  {} {} frames {frames}: {counts:?}", c.name, an.name);
            }
        }
    }
}

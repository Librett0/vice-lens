//! A map data resource holding several interiors (MLO definitions, blocks
//! `0x10506455`, 544 bytes each): each one's hash, bounds and array fields,
//! and its entities' unknown fields (`+0x70`, `+0x78`, `+0x7c`) with the
//! interiors whose bounds hold them, to tell which entity is whose.
//!
//! `mlosplit <levels.rpf> <offset-hex>`

use rage::le;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = rage::Archive::open(&a[0]).unwrap();
    let r = ar.resource_at(u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap()).unwrap();
    let blocks = rage::mapdata::blocks(&r.virt, &r.phys);
    let mut boxes = Vec::new();
    for (t, b) in &blocks {
        if *t != 0x1050_6455 {
            continue;
        }
        for m in b.as_chunks::<544>().0 {
            let f = |o: usize| f32::from_bits(le::u32(m, o));
            let lo = [f(0x40), f(0x44), f(0x48)];
            let hi = [f(0x4c), f(0x50), f(0x54)];
            let arrays: Vec<String> = (0..6).map(|k| format!("{:08x}x{}", le::u32(m, 0xb0 + k * 16), le::u16(m, 0xb8 + k * 16))).collect();
            println!("MLO {:016x} lod {:.0} box {lo:?}..{hi:?} arrays {}", le::u64(m, 8), f(0x38), arrays.join(" "));
            boxes.push((lo, hi));
        }
    }
    let mut counts = std::collections::BTreeMap::new();
    for (t, b) in &blocks {
        if *t != rage::mapdata::ENTITY {
            continue;
        }
        for e in b.as_chunks::<144>().0 {
            let f = |o: usize| f32::from_bits(le::u32(e, o));
            let p = [f(0x38), f(0x3c), f(0x40)];
            let inside: Vec<usize> = boxes.iter().enumerate().filter(|(_, (lo, hi))| (0..3).all(|c| p[c] >= lo[c] - 0.5 && p[c] <= hi[c] + 0.5)).map(|x| x.0).collect();
            *counts.entry((le::u32(e, 0x70), le::u32(e, 0x7c), format!("{inside:?}"))).or_insert(0) += 1;
        }
    }
    for ((f70, f7c, inside), n) in counts {
        println!("  +0x70 {f70:#x} +0x7c {f7c:#010x} inside {inside}: {n}");
    }
}

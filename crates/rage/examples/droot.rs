//! Print a drawable resource's root fields: the pointers `drawable::base`
//! and `lod0` follow, to see why one does not parse.
//!
//! `droot <archive> <offset-hex>`

use rage::Archive;
use rage::rsc::vptr;

fn u64at(v: &[u8], o: usize) -> u64 {
    v.get(o..o + 8).map_or(0, |s| u64::from_le_bytes(s.try_into().unwrap()))
}

fn u16at(v: &[u8], o: usize) -> u16 {
    v.get(o..o + 2).map_or(0, |s| u16::from_le_bytes(s.try_into().unwrap()))
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let archive = Archive::open(&a[0]).unwrap();
    let off = u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap();
    let r = archive.resource_at(off).unwrap();
    let v = &r.virt;
    println!("virt {} bytes, phys {} bytes, {} streams", v.len(), r.phys.len(), r.streams.len());
    if let Some(map) = vptr(u64at(v, 8)) {
        println!("block map at {map:#x}: {:02x?}", &v[map..(map + 32).min(v.len())]);
    }
    let sizes: Vec<String> = r.streams.iter().map(|s| format!("{}@{:#x}", s.size, s.offset)).collect();
    println!("first streams: {}", sizes.iter().take(4).cloned().collect::<Vec<_>>().join(" "));
    println!("last streams: {}", sizes.iter().rev().take(4).cloned().collect::<Vec<_>>().join(" "));
    if let Some(last) = r.streams.last() {
        let next: Vec<String> = {
            let mut n = Vec::new();
            archive.scan_from(last.offset + last.packed as u64, |s, _| {
                n.push(format!("{}@{:#x}", s.size, s.offset));
                n.len() < 4
            });
            n
        };
        println!("streams after it: {}", next.join(" "));
    }
    for row in 0..12 {
        let o = row * 16;
        println!("{o:#05x}: {:016x} {:016x}", u64at(v, o), u64at(v, o + 8));
    }
    for (label, b) in [("root", Some(0usize)), ("fragment +0x18", vptr(u64at(v, 0x18)))] {
        let Some(b) = b else {
            println!("{label}: no pointer");
            continue;
        };
        let lod = vptr(u64at(v, b + 0x50));
        print!("{label} at {b:#x}: +0x50 = {:016x} -> {lod:x?}", u64at(v, b + 0x50));
        if let Some(l) = lod {
            let models = vptr(u64at(v, l));
            println!("  models {models:x?} count {}", u16at(v, l + 8));
            for k in 0..4 {
                println!("    lod {k}: +{:#x} = {:016x} count {}", 0x50 + k * 8, u64at(v, b + 0x50 + k * 8), vptr(u64at(v, b + 0x50 + k * 8)).map_or(0, |l| u16at(v, l + 8)));
            }
        } else {
            println!();
        }
    }
}

//! Which files reference the most names from a list (by joaat, 4-aligned).
//!
//! `hashhits <archive> <names.txt> [min-hits]`

use std::collections::HashMap;

use rage::{Archive, joaat};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let archive = Archive::open(&a[0]).expect("archive");
    let names: HashMap<u32, String> = std::fs::read_to_string(&a[1]).expect("names").lines().filter(|l| !l.is_empty()).map(|l| (joaat(l), l.to_string())).collect();
    let min: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(20);
    let mut streams = Vec::new();
    archive.scan(|s, _| {
        streams.push(s);
        true
    });
    let mut out = Vec::new();
    for f in Archive::files(&streams) {
        let Ok(d) = archive.read_file(&f) else { continue };
        let mut hits = 0;
        let mut first = Vec::new();
        for c in d.as_chunks::<4>().0 {
            if let Some(n) = names.get(&u32::from_le_bytes([c[0], c[1], c[2], c[3]])) {
                hits += 1;
                if first.len() < 4 {
                    first.push(n.clone());
                }
            }
        }
        if hits >= min {
            let head = String::from_utf8_lossy(&d[..4.min(d.len())]).replace(|c: char| !c.is_ascii_graphic(), ".");
            out.push((hits, f.offset(), d.len(), head, first));
        }
    }
    out.sort_by(|a, b| b.0.cmp(&a.0));
    for (hits, off, size, head, first) in out.iter().take(40) {
        println!("{hits:7} {off:#012x} {size:9} {head:4} {first:?}");
    }
    eprintln!("{} files with >= {min} hits", out.len());
}

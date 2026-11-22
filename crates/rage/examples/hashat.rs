//! Like hashhits, for the files at the offsets listed (one hex offset per
//! line, first column).
//!
//! `hashat <archive> <offsets.txt> <names.txt> [min-hits]`

use std::collections::HashMap;

use rage::{Archive, joaat};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let archive = Archive::open(&a[0]).expect("archive");
    let names: HashMap<u32, String> = std::fs::read_to_string(&a[2]).unwrap().lines().filter(|l| !l.is_empty()).map(|l| (joaat(l), l.to_string())).collect();
    let min: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(10);
    let mut out = Vec::new();
    for line in std::fs::read_to_string(&a[1]).unwrap().lines() {
        let Some(off) = line.split_whitespace().next().and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok()) else { continue };
        let Ok(d) = archive.file_at(off) else { continue };
        let hits: Vec<&String> = d.as_chunks::<4>().0.iter().filter_map(|c| names.get(&u32::from_le_bytes([c[0], c[1], c[2], c[3]]))).collect();
        if hits.len() >= min {
            out.push((hits.len(), off, d.len(), hits.iter().take(3).map(|s| s.to_string()).collect::<Vec<_>>()));
        }
    }
    out.sort_by(|a, b| b.0.cmp(&a.0));
    for (n, off, size, first) in out.iter().take(60) {
        println!("{n:7} {off:#014x} {size:9} {first:?}");
    }
    eprintln!("{} files with >= {min} hits", out.len());
}

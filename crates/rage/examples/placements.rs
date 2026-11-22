//! Files that look like placement data: a model name hash followed (8 bytes
//! later) by three floats inside the world, as in the door records.
//!
//! `placements <archive> <offsets.txt|all> <names.txt> [min]`

use std::collections::HashMap;

use rage::{Archive, joaat};

fn world(b: &[u8], o: usize) -> bool {
    if o + 12 > b.len() {
        return false;
    }
    let f = |k: usize| f32::from_le_bytes(b[o + k * 4..o + k * 4 + 4].try_into().unwrap());
    let (x, y, z) = (f(0), f(1), f(2));
    x.is_finite() && y.is_finite() && z.is_finite() && x.abs() < 14000.0 && y.abs() < 14000.0 && (-80.0..900.0).contains(&z) && (x.abs() > 1.0 || y.abs() > 1.0)
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let names: HashMap<u32, String> = std::fs::read_to_string(&a[2]).unwrap().lines().filter(|l| !l.is_empty()).map(|l| (joaat(l), l.to_string())).collect();
    let min: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(5);
    let mut offsets = Vec::new();
    if a[1] == "all" {
        let mut streams = Vec::new();
        ar.scan(|s, _| {
            streams.push(s);
            true
        });
        offsets = Archive::files(&streams).iter().map(|f| f.offset()).collect();
    } else {
        for line in std::fs::read_to_string(&a[1]).unwrap().lines() {
            if let Some(o) = line.split_whitespace().next().and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok()) {
                offsets.push(o);
            }
        }
    }
    let mut out = Vec::new();
    for off in offsets {
        let Ok(d) = ar.file_at(off) else { continue };
        let mut hits = 0;
        let mut ex = Vec::new();
        for i in (0..d.len().saturating_sub(24)).step_by(4) {
            let h = u32::from_le_bytes(d[i..i + 4].try_into().unwrap());
            if let Some(n) = names.get(&h)
                && world(&d, i + 8)
            {
                hits += 1;
                if ex.len() < 3 {
                    ex.push(n.clone());
                }
            }
        }
        if hits >= min {
            out.push((hits, off, d.len(), String::from_utf8_lossy(&d[..4]).replace(|c: char| !c.is_ascii_graphic(), "."), ex));
        }
    }
    out.sort_by(|a, b| b.0.cmp(&a.0));
    for (h, off, size, head, ex) in out.iter().take(50) {
        println!("{h:7} {off:#014x} {size:9} {head:4} {ex:?}");
    }
    eprintln!("{} files", out.len());
}

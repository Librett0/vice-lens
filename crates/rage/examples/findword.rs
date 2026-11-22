//! Find 32-bit words in the blocks of every map data resource: where a hash
//! seen in one resource is referred to from others.
//!
//! `findword <levels.rpf> <survey-levels.tsv> <threads> <word-hex>...`;
//! with `KIND=PSO` (or another survey kind), the files of that kind,
//! read whole, instead.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = rage::Archive::open(&a[0]).unwrap();
    let rows = rage::index::read_survey(&std::fs::read_to_string(&a[1]).unwrap());
    let threads: usize = a[2].parse().unwrap();
    let words: Vec<u32> = a[3..].iter().map(|w| u32::from_str_radix(w.trim_start_matches("0x"), 16).unwrap()).collect();
    let kind = std::env::var("KIND").ok();
    let offs: Vec<u64> = match &kind {
        Some(k) => rows.iter().filter(|r| r.kind.starts_with(k.as_str())).map(|r| r.offset).collect(),
        None => rage::index::map_data(&rows),
    };
    let next = AtomicUsize::new(0);
    let hits = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                while let Some(&off) = offs.get(next.fetch_add(1, Ordering::Relaxed)) {
                    if kind.is_some() {
                        let Ok(d) = ar.file_at(off) else { continue };
                        for k in 0..d.len().saturating_sub(3) {
                            let v = u32::from_le_bytes(d[k..k + 4].try_into().unwrap());
                            if words.contains(&v) {
                                hits.lock().unwrap().push((v, off, 0, k, d.len()));
                            }
                        }
                        continue;
                    }
                    let Ok(r) = ar.resource_at(off) else { continue };
                    for (t, b) in rage::mapdata::blocks(&r.virt, &r.phys) {
                        for (k, w) in b.as_chunks::<4>().0.iter().enumerate() {
                            let v = u32::from_le_bytes(*w);
                            if words.contains(&v) {
                                hits.lock().unwrap().push((v, off, t, k * 4, b.len()));
                            }
                        }
                    }
                }
            });
        }
    });
    let mut h = hits.into_inner().unwrap();
    h.sort();
    for (v, off, t, at, len) in h {
        println!("{v:08x} in {off:#x} block {t:08x} +{at:#x} (block {len} bytes)");
    }
}

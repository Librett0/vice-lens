//! Find drawables the model index missed: resources not in it whose root
//! parses as a drawable. Prints `archive<TAB>offset<TAB>name` lines that can
//! be appended to models.tsv.
//!
//! `unnamed <game> <archive> <survey.tsv> <models.tsv> <out.tsv> [threads]`
use rage::{Archive, drawable};
use std::collections::HashSet;
use std::io::Write;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let name = a[1].clone();
    let ar = Archive::open(std::path::Path::new(&a[0]).join(&name)).unwrap();
    let models: HashSet<u64> = std::fs::read_to_string(&a[3])
        .unwrap()
        .lines()
        .filter(|l| l.split('\t').next() == Some(name.as_str()))
        .filter_map(|l| u64::from_str_radix(l.split('\t').nth(1)?.trim_start_matches("0x"), 16).ok())
        .collect();
    let offs: Vec<u64> = std::fs::read_to_string(&a[2])
        .unwrap()
        .lines()
        .filter(|l| l.split('\t').nth(3).is_some_and(|k| k.starts_with("RSC")))
        .filter_map(|l| u64::from_str_radix(l.split('\t').next()?.trim_start_matches("0x"), 16).ok())
        .filter(|o| !models.contains(o))
        .collect();
    eprintln!("{} resources to check", offs.len());
    let out = Mutex::new(std::io::BufWriter::new(std::fs::File::create(&a[4]).unwrap()));
    let (next, found) = (AtomicUsize::new(0), AtomicUsize::new(0));
    let threads: usize = a.get(5).and_then(|s| s.parse().ok()).unwrap_or(8);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&off) = offs.get(i) else { break };
                    // Cheap check on the first stream before reading it all.
                    let Ok((first, _)) = ar.inflate_at(off, 1 << 20) else { continue };
                    if first.len() < 0x100 || rage::le_u64(&first, 0) != 0 {
                        continue;
                    }
                    // Not a drawable (or fragment) when its LOD table lies in the
                    // first stream and does not parse.
                    let lod_at = |b: usize| rage::rsc::vptr(rage::le_u64(&first, b + 0x50));
                    let frag = rage::rsc::vptr(rage::le_u64(&first, 0x18)).filter(|&f| f + 0x60 < first.len());
                    let in_first = |b: usize| lod_at(b).is_some_and(|l| l + 16 < first.len());
                    if drawable::base(&first).is_none() && in_first(0) && frag.is_none_or(in_first) {
                        continue;
                    }
                    let Ok(r) = ar.resource_at(off) else { continue };
                    let Some(b) = drawable::base(&r.virt) else { continue };
                    let n = drawable::name(&r.virt, b);
                    if n.is_empty() {
                        continue;
                    }
                    writeln!(out.lock().unwrap(), "{name}\t{off:#x}\t{n}").unwrap();
                    found.fetch_add(1, Ordering::Relaxed);
                }
            });
        }
    });
    eprintln!("{} drawables", found.load(Ordering::Relaxed));
}

//! Whether texture keys are in the game's texture packs, and what each is:
//! size, format, pack. `ttpfkeys <game dir> <key-hex>...`
use std::path::{Path, PathBuf};
fn collect(d: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(d) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(&p, out);
        } else if p.extension().is_some_and(|x| x == "ttpf") {
            out.push(p);
        }
    }
}
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let keys: Vec<u64> = a[1..].iter().map(|k| u64::from_str_radix(k.trim_start_matches("0x"), 16).unwrap()).collect();
    let mut paths = Vec::new();
    collect(Path::new(&a[0]), &mut paths);
    let mut found = vec![false; keys.len()];
    for p in paths {
        let Ok(pack) = rage::ttpf::Pack::open(&p) else { continue };
        for (i, e) in pack.entries.iter().enumerate() {
            if let Some(k) = keys.iter().position(|&k| k == e.key) {
                found[k] = true;
                println!("{:016x}: {}x{} dxgi {} plain {} in {} #{i}", e.key, e.width, e.height, e.dxgi, pack.top_plain(i), p.display());
            }
        }
    }
    for (k, f) in keys.iter().zip(found) {
        if !f {
            println!("{k:016x}: not in the packs");
        }
    }
}

//! A dictionary's motions: per motion its clips (by name hash, and their
//! dictionary's) and each clip's parameters, first and last key.
//!
//! `motiondump <archive> <dictionary offset>`
use rage::{Archive, anim};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let v = ar.resource_at(u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap()).unwrap().virt;
    for m in anim::motions(&v) {
        println!("{} ({} clips)", m.name, m.sources.len());
        for s in &m.sources {
            let p: Vec<String> = s.params.iter().map(|k| format!("{} keys {:?}→{:?}", k.len(), k.first(), k.last())).collect();
            println!("  clip {:08x} in {:08x}: {}", s.clip, s.dictionary, p.join("; "));
        }
    }
}

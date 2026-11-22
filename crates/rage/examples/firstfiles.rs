//! The first files of an archive and what they look like, with a count of
//! kinds: a quick look at an archive no cache lists yet.
//!
//! `firstfiles <archive> <count>`
use rage::Archive;
use std::collections::BTreeMap;
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let n: usize = a[1].parse().unwrap();
    let mut streams = Vec::new();
    ar.scan(|s, _| {
        streams.push(s);
        streams.len() < n * 4
    });
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    for (i, f) in Archive::files(&streams).iter().take(n).enumerate() {
        let Ok(d) = ar.read_file(f) else { continue };
        let k = rage::index::kind(&d);
        *kinds.entry(k.split(' ').next().unwrap_or("").to_string()).or_default() += 1;
        if i < 25 {
            println!("{:#x} {:8} {}", f.offset(), d.len(), k.chars().take(120).collect::<String>());
        }
    }
    println!("{kinds:?}");
}

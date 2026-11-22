//! Block types of the `0DRPz` resources in an archive, tallied: how many
//! files carry each type, with sizes and an example file, to tell kinds of
//! data apart in an archive not read yet. Other files are tallied by kind.
//!
//! `drpztypes <archive> [max files]`
use std::collections::BTreeMap;

use rage::Archive;
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let max: usize = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(usize::MAX);
    let mut streams = Vec::new();
    ar.scan(|s, _| {
        streams.push(s);
        true
    });
    // type → (files, bytes, example offset)
    let mut types: BTreeMap<u32, (usize, usize, u64)> = BTreeMap::new();
    let mut kinds: BTreeMap<String, (usize, u64)> = BTreeMap::new();
    for f in Archive::files(&streams).iter().take(max) {
        let Ok(d) = ar.read_file(f) else { continue };
        if !rage::text::is_text_table(&d) {
            let k = rage::index::kind(&d);
            let k: String = k.split(' ').take(2).collect::<Vec<_>>().join(" ");
            kinds.entry(k).or_insert((0, f.offset())).0 += 1;
            continue;
        }
        let Ok(r) = ar.resource_at(f.offset()) else { continue };
        for (t, b) in rage::mapdata::blocks(&r.virt, &r.phys) {
            let e = types.entry(t).or_insert((0, 0, f.offset()));
            e.0 += 1;
            e.1 += b.len();
        }
    }
    let mut v: Vec<_> = types.into_iter().collect();
    v.sort_by_key(|x| std::cmp::Reverse(x.1.0));
    for (t, (n, bytes, ex)) in v.iter().take(60) {
        println!("{t:08x} {n:7} files {:10} bytes  e.g. {ex:#x}", bytes);
    }
    let mut k: Vec<_> = kinds.into_iter().collect();
    k.sort_by_key(|x| std::cmp::Reverse(x.1.0));
    for (name, (n, ex)) in k.iter().take(30) {
        println!("{n:7} {name}  e.g. {ex:#x}");
    }
}

//! The strings inside an archive's resources (other than `0DRPz` data):
//! per resource its size and first strings, and a tally of the endings
//! (`.iff`, `.clip`…) the strings carry, to tell kinds of resources apart.
//!
//! `rscstrings <archive> [max files] [print]`
use std::collections::BTreeMap;

use rage::Archive;
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let max: usize = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(usize::MAX);
    let print: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(40);
    let mut streams = Vec::new();
    ar.scan(|s, _| {
        streams.push(s);
        true
    });
    let mut ends: BTreeMap<String, (usize, usize, u64)> = BTreeMap::new();
    let mut shown = 0;
    for f in Archive::files(&streams).iter().take(max) {
        let Ok(d) = ar.read_file(f) else { continue };
        if rage::text::is_text_table(&d) || !rage::rsc::looks_like_resource(&d) {
            continue;
        }
        // A small resource packed with others is read whole as the file.
        let r = ar.resource_at(f.offset()).unwrap_or(rage::rsc::Resource { virt: d.clone(), phys: Vec::new(), streams: Vec::new() });
        let mut strs = Vec::new();
        let mut cur = Vec::new();
        for &c in &r.virt {
            if (0x20..0x7f).contains(&c) {
                cur.push(c);
            } else {
                if cur.len() >= 6 && cur.iter().filter(|c| c.is_ascii_alphabetic()).count() * 2 > cur.len() {
                    strs.push(String::from_utf8_lossy(&cur).to_string());
                }
                cur.clear();
            }
        }
        let end = strs.iter().find_map(|s| s.rfind('.').map(|i| s[i..].to_string())).unwrap_or_else(|| "(none)".into());
        let e = ends.entry(end).or_insert((0, 0, f.offset()));
        e.0 += 1;
        e.1 += r.virt.len() + r.phys.len();
        if shown < print {
            shown += 1;
            println!("{:#x} v{} p{} {}", f.offset(), r.virt.len(), r.phys.len(), strs.iter().take(3).cloned().collect::<Vec<_>>().join(" | "));
        }
    }
    let mut v: Vec<_> = ends.into_iter().collect();
    v.sort_by_key(|x| std::cmp::Reverse(x.1.0));
    for (e, (n, bytes, ex)) in v.iter().take(30) {
        println!("{n:7} {:10} bytes  {e}  e.g. {ex:#x}", bytes);
    }
}

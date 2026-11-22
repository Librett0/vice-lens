//! The dictionaries of `anim-dicts.tsv` that hold no clips: each entry's
//! name (`+0x30`), the clips it names (`+0x60`, 16 bytes each: clip and
//! dictionary name hashes at `+8` and `+12`) and how many of those
//! dictionaries the list knows, on four threads.
//!
//! `motions <anim.rpf> <anim-dicts.tsv> [samples]`
use std::collections::HashMap;

use rage::{Archive, le, rsc::vptr};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let samples: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(20);
    let list = std::fs::read_to_string(&a[1]).unwrap();
    let mut by_hash: HashMap<u32, (&str, usize)> = HashMap::new();
    let mut rows = Vec::new();
    for l in list.lines() {
        let mut f = l.split('\t');
        let (Some(o), Some(name), Some(n)) = (f.next(), f.next(), f.next()) else { continue };
        let n: usize = n.parse().unwrap_or(0);
        by_hash.insert(rage::joaat(name), (name, n));
        if n == 0 {
            rows.push((u64::from_str_radix(o.trim_start_matches("0x"), 16).unwrap(), name));
        }
    }
    let by_hash = &by_hash;
    // Per thread: entries, clips named, of those in listed dictionaries,
    // dictionaries with no entry, samples, entries by name ending.
    type Tally = (usize, usize, usize, usize, Vec<String>, HashMap<String, usize>);
    let out: Vec<Tally> = std::thread::scope(|s| {
        let hs: Vec<_> = rows
            .chunks(rows.len().div_ceil(4))
            .map(|chunk| {
                let path = a[0].clone();
                s.spawn(move || {
                    let ar = Archive::open(&path).unwrap();
                    let (mut entries, mut refs, mut known, mut empty) = (0, 0, 0, 0);
                    let mut shown = Vec::new();
                    let mut tails: HashMap<String, usize> = HashMap::new();
                    for &(o, dname) in chunk {
                        let Ok(r) = ar.resource_at(o) else { continue };
                        let v = r.virt;
                        let Some(map) = vptr(le::u64(&v, 0x48)) else { continue };
                        let mut any = false;
                        for b in 0..le::u16(&v, 0x40) as usize {
                            let mut e = vptr(le::u64(&v, map + b * 8));
                            while let Some(eo) = e.filter(|&eo| eo + 0x18 <= v.len()) {
                                e = vptr(le::u64(&v, eo + 0x10));
                                let Some(m) = vptr(le::u64(&v, eo + 8)).filter(|&m| m + 0xc0 <= v.len()) else { continue };
                                let name = vptr(le::u64(&v, m + 0x30)).filter(|&p| p < v.len()).map(|p| String::from_utf8_lossy(le::cstr(&v, p)).into_owned()).unwrap_or_default();
                                let tail = name.rsplit('.').next().unwrap_or("").to_string();
                                *tails.entry(tail).or_default() += 1;
                                entries += 1;
                                any = true;
                                let (lp, n) = (vptr(le::u64(&v, m + 0x60)), le::u16(&v, m + 0x68) as usize);
                                let mut dicts: HashMap<&str, usize> = HashMap::new();
                                if let Some(lp) = lp.filter(|&lp| lp + n * 16 <= v.len()) {
                                    for k in 0..n {
                                        refs += 1;
                                        let dh = le::u32(&v, lp + k * 16 + 12);
                                        if let Some((dn, _)) = by_hash.get(&dh) {
                                            known += 1;
                                            *dicts.entry(dn).or_default() += 1;
                                        } else {
                                            *dicts.entry("?").or_default() += 1;
                                        }
                                    }
                                }
                                if shown.len() < samples {
                                    let d: Vec<String> = dicts.iter().map(|(k, n)| format!("{k} ×{n}")).collect();
                                    shown.push(format!("{dname}: {name} → {n} clips in {}", d.join(", ")));
                                }
                            }
                        }
                        if !any {
                            empty += 1;
                        }
                    }
                    (entries, refs, known, empty, shown, tails)
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let (mut entries, mut refs, mut known, mut empty) = (0, 0, 0, 0);
    let mut tails: HashMap<String, usize> = HashMap::new();
    for (e, r, k, em, shown, t) in out {
        entries += e;
        refs += r;
        known += k;
        empty += em;
        for s in shown {
            println!("{s}");
        }
        for (k, n) in t {
            *tails.entry(k).or_default() += n;
        }
    }
    println!("{} dictionaries without clips ({empty} with no entry): {entries} entries {tails:?}, naming {refs} clips, {known} in dictionaries listed", rows.len());
}

//! Every clip of the dictionaries listed in `anim-dicts.tsv` (offset, name,
//! clips per line) decoded and sampled at 30 a second, on four threads: how
//! many play, and per bone (by name hash) how many clips give it turns that
//! are not unit quaternions or places farther than 50 m from its parent.
//!
//! `animcheck <game folder> <anim-dicts.tsv> [archive]`: the archive
//! column names a content pack's (`anim.rpf` when there is none); with
//! `archive`, only its dictionaries.
use std::collections::HashMap;

use rage::{Archive, anim};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let list = std::fs::read_to_string(&a[1]).unwrap();
    let only = a.get(2).cloned();
    let rows: Vec<(u64, &str, &str)> = list
        .lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            let (o, name, n) = (f.first()?, f.get(1)?, f.get(2)?);
            let arch = f.get(4).copied().unwrap_or("anim.rpf");
            (*n != "0" && only.as_deref().is_none_or(|x| x == arch)).then(|| (u64::from_str_radix(o.trim_start_matches("0x"), 16).unwrap(), *name, arch))
        })
        .collect();
    // Per thread: dictionaries, clips, partial clips, odd clips per bone
    // (count and a clip), unread dictionaries.
    type Tally = (usize, usize, usize, HashMap<u32, (usize, String)>, usize);
    let parts: Vec<Tally> = std::thread::scope(|s| {
        let handles: Vec<_> = rows
            .chunks(rows.len().div_ceil(4))
            .map(|chunk| {
                let game = std::path::PathBuf::from(&a[0]);
                s.spawn(move || {
                    let mut open: HashMap<&str, Archive> = HashMap::new();
                    let mut t: Tally = Default::default();
                    for (k, &(o, name, arch)) in chunk.iter().enumerate() {
                        let ar = open.entry(arch).or_insert_with(|| Archive::open(game.join(arch)).unwrap());
                        if k % 2000 == 0 {
                            eprintln!("{k} of {}", chunk.len());
                        }
                        let Some(r) = ar.resource_at(o).ok() else {
                            t.4 += 1;
                            continue;
                        };
                        let Some((_, cs)) = anim::dictionary(&r.virt) else {
                            t.4 += 1;
                            continue;
                        };
                        t.0 += 1;
                        for c in &cs {
                            t.1 += 1;
                            if !anim::readable(&r.virt, c) {
                                t.2 += 1;
                                continue;
                            }
                            let (_, poses) = anim::sample(&r.virt, c, 30.0);
                            for (h, ps) in &poses {
                                let bad = ps.iter().any(|p| {
                                    p.rotation.is_some_and(|q| !q.iter().all(|x| x.is_finite()) || (q.iter().map(|x| x * x).sum::<f32>() - 1.0).abs() > 0.01)
                                        || p.position.is_some_and(|t| !t.iter().all(|x| x.is_finite() && x.abs() < 50.0))
                                });
                                if bad {
                                    let e = t.3.entry(*h).or_insert((0, format!("{name} {}", c.name)));
                                    e.0 += 1;
                                }
                            }
                        }
                    }
                    t
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let (mut dicts, mut clips, mut partial, mut unread) = (0, 0, 0, 0);
    let mut odd: HashMap<u32, (usize, String)> = HashMap::new();
    for t in parts {
        dicts += t.0;
        clips += t.1;
        partial += t.2;
        unread += t.4;
        for (h, (n, e)) in t.3 {
            odd.entry(h).or_insert((0, e)).0 += n;
        }
    }
    println!("{dicts} dictionaries read ({unread} not), {clips} clips, {partial} partial");
    let mut odd: Vec<_> = odd.into_iter().collect();
    odd.sort_by_key(|x| std::cmp::Reverse(x.1.0));
    for (h, (n, e)) in odd {
        println!("{h:08x} odd in {n} clips, e.g. {e}");
    }
}

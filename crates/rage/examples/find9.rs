//! Clip dictionaries (from `anim-dicts.tsv`) whose blocks hold channels of
//! type 9, and clips that do not decode whole (`anim::readable`), on four
//! threads, a few per thread.
//!
//! `find9 <anim.rpf> <anim-dicts.tsv> [per thread]`
use rage::{Archive, anim, le};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let most: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(5);
    let list = std::fs::read_to_string(&a[1]).unwrap();
    let rows: Vec<(u64, &str)> = list
        .lines()
        .filter_map(|l| {
            let mut f = l.split('\t');
            let (o, name, n) = (f.next()?, f.next()?, f.next()?);
            (n != "0").then(|| (u64::from_str_radix(o.trim_start_matches("0x"), 16).unwrap(), name))
        })
        .collect();
    std::thread::scope(|s| {
        for chunk in rows.chunks(rows.len().div_ceil(4)) {
            let path = a[0].clone();
            s.spawn(move || {
                let ar = Archive::open(&path).unwrap();
                let mut found = 0;
                for &(o, name) in chunk {
                    let Ok(r) = ar.resource_at(o) else { continue };
                    let v = r.virt;
                    let Some((_, clips)) = anim::dictionary(&v) else { continue };
                    let mut seen = std::collections::HashSet::new();
                    for c in &clips {
                        if !anim::readable(&v, c) {
                            let why: Vec<String> = c
                                .parts
                                .iter()
                                .map(|p| match anim::animation(&v, p.animation) {
                                    None => format!("{:#x}: no animation", p.animation),
                                    Some(an) => format!("{:#x} {} frames {} blocks {}", p.animation, an.name, an.frames, an.blocks.len()),
                                })
                                .collect();
                            println!("unreadable {o:#x}\t{name}\t{} {:.3} s\t{}", c.name, c.length, why.join("; "));
                            found += 1;
                        }
                        for p in &c.parts {
                            if !seen.insert(p.animation) {
                                continue;
                            }
                            let Some(an) = anim::animation(&v, p.animation) else { continue };
                            for &b in &an.blocks {
                                let len = le::u32(&v, b + 4) as usize;
                                let (fo, fr, fl) = (le::u32(&v, b + 12) as usize, le::u16(&v, b + 0x16) as usize, le::u16(&v, b + 0x18) as usize);
                                let Some(d) = v.get(b + 0x20..(b + 0x20 + len).min(v.len())) else { continue };
                                let list = fo + fl * fr;
                                if list + 22 > d.len() {
                                    continue;
                                }
                                let counts: Vec<u16> = (0..11).map(|t| le::u16(d, list + t * 2)).collect();
                                if counts[9] > 0 {
                                    println!("{o:#x}\t{name}\t{}\t{:#x}\tframes {fr}\t{counts:?}", c.name, p.animation);
                                    found += 1;
                                }
                            }
                        }
                    }
                    if found >= most {
                        break;
                    }
                }
            });
        }
    });
}

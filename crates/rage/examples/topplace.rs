//! Where the top level of each mipmapped TTPF texture is read: at the end
//! of its allocation (mips smallest first) or at the start. Counts the
//! textures a bound of 1.65 times the top level sent to the start, and
//! lists some, to check them by eye (`dump key`).
//!
//! `topplace <game dir>`

use std::collections::BTreeMap;

fn main() {
    let dir = std::env::args().nth(1).unwrap();
    let mut stack = vec![std::path::PathBuf::from(dir)];
    let (mut moved, mut still, mut total) = (BTreeMap::<String, usize>::new(), BTreeMap::<String, usize>::new(), 0usize);
    let mut examples = Vec::new();
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if !p.extension().is_some_and(|x| x == "ttpf") {
                continue;
            }
            let Ok(pk) = rage::ttpf::Pack::open(&p) else { continue };
            for (i, e) in pk.entries.iter().enumerate() {
                let Some(f) = rage::swizzle::format(e.dxgi) else { continue };
                if !e.mips || !f.block {
                    continue;
                }
                total += 1;
                let size = rage::swizzle::tiled_size(f, e.width, e.height, rage::swizzle::Tile::K4) as u64;
                let old_start = !(e.alloc > size && (e.alloc as f64) <= size as f64 * 1.65);
                let key = format!("{}x{} dxgi {}", e.width, e.height, e.dxgi);
                if old_start && !pk.top_at_start(i) {
                    *moved.entry(key).or_default() += 1;
                    if examples.len() < 12 && pk.top_plain(i) && i % 7 == 0 {
                        examples.push(format!("{:016x} {}x{} dxgi {}", e.key, e.width, e.height, e.dxgi));
                    }
                } else if pk.top_at_start(i) && e.alloc > size {
                    *still.entry(key).or_default() += 1;
                }
            }
        }
    }
    let sum = |m: &BTreeMap<String, usize>| m.values().sum::<usize>();
    println!("{total} mipmapped block textures; now read at the end, were at the start: {}; still at the start with room to spare: {}", sum(&moved), sum(&still));
    let mut v: Vec<_> = moved.into_iter().collect();
    v.sort_by_key(|x| std::cmp::Reverse(x.1));
    println!("moved, by size and format: {:?}", &v[..v.len().min(15)]);
    let mut s: Vec<_> = still.into_iter().collect();
    s.sort_by_key(|x| std::cmp::Reverse(x.1));
    println!("still at the start: {:?}", &s[..s.len().min(10)]);
    for e in examples {
        println!("  {e}");
    }
}

//! Survey: the vertex-buffer field at +0x6c against the axis order that
//! makes a drawable's decoded extent match its declared bounds.
//!
//! `axes <game> <models.tsv> <count>`
use rage::{Archive, drawable, le};
use std::collections::{BTreeMap, HashMap};
const PERMS: [[usize; 3]; 6] = [[0, 1, 2], [1, 2, 0], [2, 0, 1], [0, 2, 1], [1, 0, 2], [2, 1, 0]];
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let game = std::path::Path::new(&a[0]);
    let list = std::fs::read_to_string(&a[1]).unwrap();
    let count: usize = a[2].parse().unwrap();
    let mut archives: HashMap<String, Archive> = HashMap::new();
    let mut hist: BTreeMap<(u32, String), usize> = BTreeMap::new();
    let mut seen = 0;
    for (li, line) in list.lines().enumerate() {
        if seen >= count {
            break;
        }
        if li % 97 != 0 {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 3 {
            continue;
        }
        let ar = archives.entry(f[0].to_string()).or_insert_with(|| Archive::open(game.join(f[0])).unwrap());
        let off = u64::from_str_radix(f[1].trim_start_matches("0x"), 16).unwrap();
        let Ok(res) = ar.resource_at(off) else { continue };
        let Some(d) = drawable::parse(&res.virt, &res.phys) else { continue };
        let decl = [d.max[0] - d.min[0], d.max[1] - d.min[1], d.max[2] - d.min[2]];
        // Distinct extents, else the order cannot be told.
        let mut s = decl;
        s.sort_by(|a, b| a.partial_cmp(b).unwrap());
        if s[0] < 0.05 || s[1] / s[0] < 1.3 || s[2] / s[1] < 1.3 {
            continue;
        }
        for m in &d.meshes {
            let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
            for p in &m.positions {
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
            let ext = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
            // Only meshes spanning nearly the whole drawable.
            let mut best = None;
            for (pi, pm) in PERMS.iter().enumerate() {
                // slot j holds true axis pm[j]
                let ok = (0..3).all(|j| (ext[j] - decl[pm[j]]).abs() <= 0.03 * decl[pm[j]].max(0.1));
                if ok {
                    best = Some(pi);
                }
            }
            let k = le::u32(&res.phys, m.vb + 0x6c);
            let key = match best {
                Some(pi) => {
                    let pm = PERMS[pi];
                    // Sign per slot: + if the slot's range matches the axis
                    // range, - if it matches it negated.
                    let sg: String = (0..3)
                        .map(|j| {
                            let (mn, mx) = (d.min[pm[j]], d.max[pm[j]]);
                            let tol = 0.05 * (mx - mn).max(0.1);
                            let pos = (lo[j] - mn).abs() < tol && (hi[j] - mx).abs() < tol;
                            let neg = (-hi[j] - mn).abs() < tol && (-lo[j] - mx).abs() < tol;
                            match (pos, neg) {
                                (true, true) => '=',
                                (true, false) => '+',
                                (false, true) => '-',
                                _ => '?',
                            }
                        })
                        .collect();
                    format!("{:?} {sg}", pm)
                }
                None => "partial".into(),
            };
            if best == Some(4) || std::env::var("AXES_ALL").is_ok() && best.is_some() {
                let w: Vec<String> = (0x60..0x80).step_by(4).map(|o| format!("{:08x}", le::u32(&res.phys, m.vb + o))).collect();
                println!("{} k={k} {key} decl {decl:.2?} ext {ext:.2?} {}", f[2], w.join(" "));
            }
            if best.is_some() {
                *hist.entry((k, key)).or_default() += 1;
            }
        }
        seen += 1;
    }
    println!("{seen} drawables");
    for ((k, p), n) in hist {
        println!("+0x6c={k:<3} slots->axes {p:<10} {n}");
    }
}

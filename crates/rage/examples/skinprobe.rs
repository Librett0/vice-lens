//! Which per-vertex arrays tie a model's vertices to its bones. Every
//! 4-byte array between the vertex streams is tried: as weights (bytes
//! summing to 255) and as bone indices (how near each vertex lies to the
//! bone its first byte names, against a bone picked at random).
//!
//! `skinprobe <archive> <offset> <name> [meshes]`
use rage::{Archive, drawable};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let r = ar.resource_at(u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap()).unwrap();
    let d = drawable::parse_model(&r.virt, &r.phys, rage::joaat(&a[2])).unwrap();
    let p = &r.phys;
    // The skeleton may live in another model (a creature's or person's
    // fragment): SKEL=<offset>:<name> in the same archive.
    let skel = std::env::var("SKEL").ok().map(|s| {
        let (o, n) = s.split_once(':').unwrap();
        let r = ar.resource_at(u64::from_str_radix(o.trim_start_matches("0x"), 16).unwrap()).unwrap();
        drawable::parse_model(&r.virt, &r.phys, rage::joaat(n)).unwrap().bones
    });
    let bones = skel.unwrap_or_else(|| d.bones.clone());
    let nb = bones.len();
    println!("{} bones, {} meshes", nb, d.meshes.len());
    let bpos: Vec<[f32; 3]> = bones.iter().map(|b| [b.matrix[12], b.matrix[13], b.matrix[14]]).collect();
    if std::env::var_os("LIST").is_some() {
        for (i, b) in bones.iter().enumerate() {
            println!("  bone {i:3} {:30} parent {:?} at ({:.3} {:.3} {:.3})", b.name, b.parent, b.matrix[12], b.matrix[13], b.matrix[14]);
        }
    }
    let want: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(2);
    for (mi, m) in d.meshes.iter().enumerate().take(want) {
        let nv = m.positions.len();
        let mut s = m.streams.clone();
        s.sort_by_key(|x| x.1);
        println!("mesh {mi}: {nv} verts, streams {:?}", s.iter().map(|x| format!("{:#04x}@{:#x}", x.0, x.1)).collect::<Vec<_>>());
        let end_all = s.last().map_or(0, |x| x.1 + nv * 4);
        for (k, &(t, o)) in s.iter().enumerate() {
            let end = s.get(k + 1).map_or(end_all, |x| x.1);
            let arrays = (end - o) / (nv * 4).max(1);
            for j in 0..arrays.max(1) {
                let at = m.vb + o + j * nv * 4;
                if at + nv * 4 > p.len() {
                    continue;
                }
                let byte = |v: usize, c: usize| p[at + v * 4 + c] as usize;
                let sum255 = (0..nv).filter(|&v| (0..4).map(|c| byte(v, c)).sum::<usize>() == 255).count();
                let mut dist = [0.0f64; 4];
                let mut ok = [0usize; 4];
                let mut rnd = 0.0f64;
                let mut seed = 12345u64;
                for v in 0..nv {
                    let q = m.positions[v];
                    for c in 0..4 {
                        let b = byte(v, c);
                        if b < nb {
                            let bp = bpos[b];
                            dist[c] += (((q[0] - bp[0]).powi(2) + (q[1] - bp[1]).powi(2) + (q[2] - bp[2]).powi(2)) as f64).sqrt();
                            ok[c] += 1;
                        }
                    }
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    let bp = bpos[(seed >> 33) as usize % nb.max(1)];
                    rnd += (((q[0] - bp[0]).powi(2) + (q[1] - bp[1]).powi(2) + (q[2] - bp[2]).powi(2)) as f64).sqrt();
                }
                let avg: Vec<String> = (0..4).map(|c| if ok[c] > 0 { format!("{:.3}({}%)", dist[c] / ok[c] as f64, ok[c] * 100 / nv.max(1)) } else { "-".into() }).collect();
                // Paired with the weights array (WEIGHTS=<hex offset>): how far
                // each vertex is from the bone its largest weight names.
                if let Some(wo) = std::env::var("WEIGHTS").ok().and_then(|w| usize::from_str_radix(w.trim_start_matches("0x"), 16).ok()) {
                    let wa = m.vb + wo;
                    let (mut dsum, mut n, mut nearest) = (0.0f64, 0usize, 0.0f64);
                    for v in 0..nv {
                        let ws: Vec<usize> = (0..4).map(|c| p[wa + v * 4 + c] as usize).collect();
                        let c = (0..4).max_by_key(|&c| ws[c]).unwrap();
                        let q = m.positions[v];
                        let dd = |bp: [f32; 3]| (((q[0] - bp[0]).powi(2) + (q[1] - bp[1]).powi(2) + (q[2] - bp[2]).powi(2)) as f64).sqrt();
                        nearest += bpos.iter().map(|&bp| dd(bp)).fold(f64::MAX, f64::min);
                        let b = byte(v, c);
                        if b < nb {
                            dsum += dd(bpos[b]);
                            n += 1;
                        }
                    }
                    println!("    with weights: to the heaviest bone {:.3} over {}% (nearest bone {:.3})", dsum / n.max(1) as f64, n * 100 / nv.max(1), nearest / nv.max(1) as f64);
                }
                println!("  {t:#04x}@{o:#x} [{j}] sum255 {:3}%  near byte0..3 {}  random {:.3}   first {:02x?}", sum255 * 100 / nv.max(1), avg.join(" "), rnd / nv.max(1) as f64, &p[at..at + 12]);
            }
        }
    }
}

//! Find how tile numbers 1..64 lay out in a 512 m block, from edge heights.
//!
//! `tiles <levels.rpf> <hd-tiles.tsv> <block prefix, e.g. n10_w05>`

use std::collections::HashMap;

use rage::{Archive, drawable};

/// Mean height of vertices within 0.5 m of an edge, sampled in 8 bins.
fn edge(ps: &[[f32; 3]], side: usize) -> Vec<Option<f32>> {
    let mut bins = [(0.0f32, 0u32); 8];
    for p in ps {
        let (on, t) = match side {
            0 => (p[1] > 31.5, p[0]),  // +y edge
            1 => (p[1] < -31.5, p[0]), // -y edge
            2 => (p[0] > 31.5, p[1]),  // +x edge
            _ => (p[0] < -31.5, p[1]), // -x edge
        };
        if on {
            let b = (((t + 32.0) / 8.0) as usize).min(7);
            bins[b].0 += p[2];
            bins[b].1 += 1;
        }
    }
    bins.iter().map(|(s, n)| (*n > 0).then(|| s / *n as f32)).collect()
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let mut tiles: HashMap<usize, Vec<[f32; 3]>> = HashMap::new();
    for line in std::fs::read_to_string(&a[1]).unwrap().lines() {
        let (off, name) = line.split_once('\t').unwrap();
        let Some(rest) = name.strip_prefix(&format!("{}_", a[2])) else { continue };
        let Some(idx) = rest.strip_suffix("_hd").and_then(|s| s.parse::<usize>().ok()) else { continue };
        let off = u64::from_str_radix(off.trim_start_matches("0x"), 16).unwrap();
        let Ok(res) = ar.resource_at(off) else { continue };
        let Some(d) = drawable::parse(&res.virt, &res.phys) else { continue };
        tiles.insert(idx, d.meshes.iter().flat_map(|m| m.positions.iter().copied()).collect());
    }
    eprintln!("{} tiles", tiles.len());
    let edges: HashMap<usize, Vec<Vec<Option<f32>>>> = tiles.iter().map(|(i, p)| (*i, (0..4).map(|s| edge(p, s)).collect())).collect();
    let diff = |a: &[Option<f32>], b: &[Option<f32>]| -> Option<f32> {
        let v: Vec<f32> = a.iter().zip(b).filter_map(|(x, y)| Some((x.as_ref()? - y.as_ref()?).abs())).collect();
        (!v.is_empty()).then(|| v.iter().sum::<f32>() / v.len() as f32)
    };
    // Hypotheses: index i -> (col, row) with col = (i-1)%8 or (i-1)/8, and
    // columns running +x or -x, rows +y or -y.
    for transposed in [false, true] {
        for xdir in [1i32, -1] {
            for ydir in [1i32, -1] {
                let pos = |i: usize| {
                    let (a, b) = (((i - 1) % 8) as i32, ((i - 1) / 8) as i32);
                    let (c, r) = if transposed { (b, a) } else { (a, b) };
                    (c * xdir, r * ydir)
                };
                let at: HashMap<(i32, i32), usize> = tiles.keys().map(|&i| (pos(i), i)).collect();
                let (mut sum, mut n) = (0.0, 0);
                for (&i, e) in &edges {
                    let (x, y) = pos(i);
                    if let Some(j) = at.get(&(x + 1, y))
                        && let Some(d) = diff(&e[2], &edges[j][3])
                    {
                        sum += d;
                        n += 1;
                    }
                    if let Some(j) = at.get(&(x, y + 1))
                        && let Some(d) = diff(&e[0], &edges[j][1])
                    {
                        sum += d;
                        n += 1;
                    }
                }
                println!("transposed {transposed} x{xdir:+} y{ydir:+}: mean edge mismatch {:.3} m over {n} edges", sum / n.max(1) as f32);
            }
        }
    }
}

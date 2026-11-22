//! UV frame statistics over drawables: on walls, which way the texture's v
//! axis points, and whether the UV mapping keeps or reverses orientation
//! relative to the outward face direction.
//!
//! `uvframe <game dir> <models.tsv> <count> [name filter]`
use rage::{Archive, drawable};
use std::collections::HashMap;

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let game = std::path::Path::new(&a[0]);
    let list = std::fs::read_to_string(&a[1]).unwrap();
    let count: usize = a[2].parse().unwrap();
    let filter = a.get(3).cloned().unwrap_or_default();
    let mut archives: HashMap<String, Archive> = HashMap::new();
    // Weighted sums: wall v pointing down / up; orientation D3D-like / GL-like.
    let (mut v_down, mut v_up, mut keep, mut flip) = (0f64, 0f64, 0f64, 0f64);
    let mut models = 0;
    let (mut ur, mut ul, mut vposc, mut vnegc) = (0f64, 0f64, 0, 0);
    for line in list.lines() {
        if models >= count {
            break;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 3 || !f[2].contains(&filter) || f[0] != "levels.rpf" {
            continue;
        }
        let ar = archives.entry(f[0].to_string()).or_insert_with(|| Archive::open(game.join(f[0])).unwrap());
        let off = u64::from_str_radix(f[1].trim_start_matches("0x"), 16).unwrap();
        let Ok(res) = ar.resource_at(off) else { continue };
        let Some(d) = drawable::parse(&res.virt, &res.phys) else { continue };
        let c = [(d.min[0] + d.max[0]) / 2.0, (d.min[1] + d.max[1]) / 2.0, (d.min[2] + d.max[2]) / 2.0];
        let (mut md, mut mu, mut mk, mut mf) = (0f64, 0f64, 0f64, 0f64);
        let (mut vol, mut detpos, mut detneg) = (0f64, 0f64, 0f64);
        let mut walls = Vec::new();
        for m in &d.meshes {
            if m.uvs.len() != m.positions.len() {
                continue;
            }
            for t in m.indices.as_chunks::<3>().0 {
                let [i0, i1, i2] = [t[0] as usize, t[1] as usize, t[2] as usize];
                let (p0, p1, p2) = (m.positions[i0], m.positions[i1], m.positions[i2]);
                let (u0, u1, u2) = (m.uvs[i0], m.uvs[i1], m.uvs[i2]);
                let (e1, e2) = (sub(p1, p0), sub(p2, p0));
                let n = cross(e1, e2);
                let area = dot(n, n).sqrt();
                if area < 1e-4 {
                    continue;
                }
                let (du1, dv1, du2, dv2) = (u1[0] - u0[0], u1[1] - u0[1], u2[0] - u0[0], u2[1] - u0[1]);
                let det = du1 * dv2 - du2 * dv1;
                if det.abs() < 1e-9 {
                    continue;
                }
                // dP/dv
                let b = [(e2[0] * du1 - e1[0] * du2) / det, (e2[1] * du1 - e1[1] * du2) / det, (e2[2] * du1 - e1[2] * du2) / det];
                let w = area as f64;
                vol += dot(p0, cross(p1, p2)) as f64;
                if det > 0.0 {
                    detpos += w
                } else {
                    detneg += w
                }
                let tv = [(e1[0] * dv2 - e2[0] * dv1) / det, (e1[1] * dv2 - e2[1] * dv1) / det, (e1[2] * dv2 - e2[2] * dv1) / det];
                if (n[2] / area).abs() < 0.2 {
                    walls.push((n, tv, w));
                }
                if (n[2] / area).abs() < 0.2 {
                    let bz = b[2] / dot(b, b).sqrt();
                    if bz < -0.7 {
                        md += w;
                    } else if bz > 0.7 {
                        mu += w;
                    }
                }
                let fc = [(p0[0] + p1[0] + p2[0]) / 3.0, (p0[1] + p1[1] + p2[1]) / 3.0, (p0[2] + p1[2] + p2[2]) / 3.0];
                let out = dot(n, sub(fc, c)).signum();
                // T×B = n / det. D3D (v down) unmirrored: T×B points inward.
                if (det.signum() * out) < 0.0 {
                    mk += w;
                } else {
                    mf += w;
                }
            }
        }
        models += 1;
        // Outward = winding normal × sign(volume). Unmirrored: dP/du points
        // to the viewer's right, right = (-N) × Z.
        let vs = vol.signum() as f32;
        let (mut right, mut left) = (0f64, 0f64);
        for (n, tv, w) in &walls {
            let o = [n[0] * vs, n[1] * vs, n[2] * vs];
            let r = cross([-o[0], -o[1], -o[2]], [0.0, 0.0, 1.0]);
            if dot(*tv, r) > 0.0 { right += w } else { left += w }
        }
        println!(
            "    volume {:+.0} det+ {:.0}% walls u-right {:.0}% u-left {:.0}%",
            vol / 6.0,
            100.0 * detpos / (detpos + detneg),
            100.0 * right / (right + left).max(1e-9),
            100.0 * left / (right + left).max(1e-9)
        );
        ur += right;
        ul += left;
        if vol > 0.0 {
            vposc += 1
        } else {
            vnegc += 1
        }
        println!(
            "{:<48} walls v-down {:5.1}% v-up {:5.1}%   orient d3d {:5.1}% gl {:5.1}%",
            f[2],
            100.0 * md / (md + mu).max(1e-9),
            100.0 * mu / (md + mu).max(1e-9),
            100.0 * mk / (mk + mf).max(1e-9),
            100.0 * mf / (mk + mf).max(1e-9)
        );
        v_down += md;
        v_up += mu;
        keep += mk;
        flip += mf;
    }
    println!("walls u-right {:.1}% u-left {:.1}%  volume>0 in {vposc} models, <0 in {vnegc}", 100.0 * ur / (ur + ul), 100.0 * ul / (ur + ul));
    println!(
        "TOTAL {models} models: walls v-down {:.1}% v-up {:.1}%   orient d3d {:.1}% gl {:.1}%",
        100.0 * v_down / (v_down + v_up),
        100.0 * v_up / (v_down + v_up),
        100.0 * keep / (keep + flip),
        100.0 * flip / (keep + flip)
    );
}

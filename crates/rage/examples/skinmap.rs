//! Which skeleton bone each local bone index of a skinned model stands
//! for, measured: per index, the middle of the vertices it weighs most,
//! and the skeleton bones nearest it. With the mapping a rule may show
//! (every n-th bone, the deforming ones in order…).
//!
//! `skinmap <archive> <offset> <name> <indices-hex> <weights-hex>` with
//! SKEL=<offset>:<name>
use rage::{Archive, drawable};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let r = ar.resource_at(u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap()).unwrap();
    let d = drawable::parse_model(&r.virt, &r.phys, rage::joaat(&a[2])).unwrap();
    let p = &r.phys;
    let io = usize::from_str_radix(&a[3], 16).unwrap();
    let wo = usize::from_str_radix(&a[4], 16).unwrap();
    let s = std::env::var("SKEL").unwrap();
    let (o, n) = s.split_once(':').unwrap();
    let sr = ar.resource_at(u64::from_str_radix(o.trim_start_matches("0x"), 16).unwrap()).unwrap();
    let bones = drawable::parse_model(&sr.virt, &sr.phys, rage::joaat(n)).unwrap().bones;
    let m = &d.meshes[0];
    let mut acc: Vec<([f64; 3], f64)> = vec![([0.0; 3], 0.0); 256];
    for v in 0..m.positions.len() {
        for c in 0..4 {
            let w = p[m.vb + wo + v * 4 + c] as f64;
            let l = p[m.vb + io + v * 4 + c] as usize;
            if w < 128.0 {
                continue;
            }
            for k in 0..3 {
                acc[l].0[k] += m.positions[v][k] as f64 * w;
            }
            acc[l].1 += w;
        }
    }
    for (l, (s, w)) in acc.iter().enumerate() {
        if *w == 0.0 {
            continue;
        }
        let c = s.map(|x| (x / w) as f32);
        let mut near: Vec<(f32, usize)> = bones.iter().enumerate().map(|(i, b)| (((c[0] - b.matrix[12]).powi(2) + (c[1] - b.matrix[13]).powi(2) + (c[2] - b.matrix[14]).powi(2)).sqrt(), i)).collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        let list: Vec<String> = near.iter().take(3).map(|(dd, i)| format!("{i}:{}({dd:.3})", bones[*i].name)).collect();
        println!("local {l:3} weight {:8.0}  at ({:.3} {:.3} {:.3})  {}", w, c[0], c[1], c[2], list.join(" "));
    }
}

//! A model's skinning as read: per bone its vertices follow (heaviest
//! weight), how many and where their middle is, by the bone's name
//! (through the palette and the skeleton `SKEL=<offset>:<name>`, else the
//! model's own skeleton by index), and how far on average each vertex lies
//! from its bone. `skincheck <archive> <offset> <name> [rows]`
use std::collections::BTreeMap;

use rage::{Archive, drawable};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let r = ar.resource_at(u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap()).unwrap();
    let d = drawable::parse_model(&r.virt, &r.phys, rage::joaat(&a[2])).unwrap();
    let rows: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(30);
    let bones = std::env::var("SKEL")
        .ok()
        .map(|s| {
            let (o, n) = s.split_once(':').unwrap();
            let r = ar.resource_at(u64::from_str_radix(o.trim_start_matches("0x"), 16).unwrap()).unwrap();
            drawable::parse_model(&r.virt, &r.phys, rage::joaat(n)).unwrap().bones
        })
        .unwrap_or_else(|| d.bones.clone());
    let by_hash: std::collections::HashMap<u32, usize> = bones.iter().enumerate().map(|(i, b)| (rage::joaat(&b.name), i)).collect();
    // bone → (vertices, sum of positions, sum of distances)
    let mut acc: BTreeMap<usize, (usize, [f64; 3], f64)> = BTreeMap::new();
    let (mut skinned, mut missing) = (0usize, 0usize);
    for m in &d.meshes {
        if m.joints.is_empty() {
            continue;
        }
        for (v, (j, w)) in m.joints.iter().zip(&m.weights).enumerate() {
            let k = (0..8).max_by_key(|&k| w[k]).unwrap();
            let bone = if m.palette.is_empty() { Some(j[k] as usize) } else { m.palette.get(j[k] as usize).and_then(|h| by_hash.get(h).copied()) };
            let Some(b) = bone.filter(|&b| b < bones.len()) else {
                missing += 1;
                continue;
            };
            skinned += 1;
            let q = m.positions[v];
            let bp = &bones[b].matrix;
            let e = acc.entry(b).or_insert((0, [0.0; 3], 0.0));
            e.0 += 1;
            for (acc, x) in e.1.iter_mut().zip(q) {
                *acc += x as f64;
            }
            e.2 += (((q[0] - bp[12]).powi(2) + (q[1] - bp[13]).powi(2) + (q[2] - bp[14]).powi(2)) as f64).sqrt();
        }
    }
    println!(
        "{} meshes, {} with bones, {skinned} vertices placed, {missing} not; palette {}",
        d.meshes.len(),
        d.meshes.iter().filter(|m| !m.joints.is_empty()).count(),
        d.meshes.iter().map(|m| m.palette.len()).max().unwrap_or(0)
    );
    let mut list: Vec<_> = acc.into_iter().collect();
    list.sort_by_key(|x| std::cmp::Reverse(x.1.0));
    let total: f64 = list.iter().map(|x| x.1.2).sum::<f64>() / list.iter().map(|x| x.1.0).sum::<usize>().max(1) as f64;
    println!("mean distance to the bone: {total:.3}");
    for (b, (n, s, dist)) in list.iter().take(rows) {
        let c = s.map(|x| x / *n as f64);
        let m = &bones[*b].matrix;
        println!("{:4} {:28} {n:7} verts  middle ({:.2} {:.2} {:.2})  bone ({:.2} {:.2} {:.2})  mean {:.3}", b, bones[*b].name, c[0], c[1], c[2], m[12], m[13], m[14], dist / *n as f64);
    }
}

//! A skinned model's palette by name: each bone hash looked up in the
//! model's own skeletons (first and further) and in a body skeleton
//! (`SKEL=<offset>:<name>`), with the own bone's parent chain.
//!
//! `palnames <archive> <offset> <name>`
use std::collections::HashMap;

use rage::{Archive, drawable};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let load = |o: &str, n: &str| {
        let r = ar.resource_at(u64::from_str_radix(o.trim_start_matches("0x"), 16).unwrap()).unwrap();
        drawable::parse_model(&r.virt, &r.phys, rage::joaat(n)).unwrap()
    };
    let d = load(&a[1], &a[2]);
    let body: HashMap<u32, String> = std::env::var("SKEL")
        .ok()
        .map(|s| {
            let (o, n) = s.split_once(':').unwrap();
            load(o, n).bones.into_iter().map(|b| (rage::joaat(&b.name), b.name)).collect()
        })
        .unwrap_or_default();
    println!("own bones {}, further {}", d.bones.len(), d.more_bones.len());
    for (i, b) in d.bones.iter().chain(&d.more_bones).enumerate() {
        println!("  own {i:3} {:<28} parent {:?} at {:.2} {:.2} {:.2}", b.name, b.parent, b.matrix[12], b.matrix[13], b.matrix[14]);
    }
    let own: Vec<&drawable::Bone> = d.bones.iter().chain(&d.more_bones).collect();
    let by: HashMap<u32, usize> = own.iter().enumerate().map(|(i, b)| (rage::joaat(&b.name), i)).collect();
    for (mi, m) in d.meshes.iter().enumerate() {
        println!("mesh {mi}: palette {}", m.palette.len());
        for (k, h) in m.palette.iter().enumerate() {
            let o = by.get(h).map(|&i| {
                let b = own[i];
                let parent = b.parent.and_then(|p| own.get(p as usize)).map_or("-".to_string(), |p| p.name.clone());
                format!("{} (parent {parent}) at {:.2} {:.2} {:.2}", b.name, b.matrix[12], b.matrix[13], b.matrix[14])
            });
            println!("  {k:3} {h:08x} body:{:<24} own:{}", body.get(h).map_or("-", |s| s.as_str()), o.unwrap_or("-".into()));
        }
    }
}

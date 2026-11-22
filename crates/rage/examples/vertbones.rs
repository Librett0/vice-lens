//! Some vertices of a skinned model as stored: position and their eight
//! bones by name (palette through the model's own skeletons) and weight.
//!
//! `vertbones <archive> <offset> <name> <mesh> <vertex>...`
use std::collections::HashMap;

use rage::{Archive, drawable};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let r = ar.resource_at(u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap()).unwrap();
    let d = drawable::parse_model(&r.virt, &r.phys, rage::joaat(&a[2])).unwrap();
    let names: HashMap<u32, &str> = d.bones.iter().chain(&d.more_bones).map(|b| (rage::joaat(&b.name), b.name.as_str())).collect();
    let m = &d.meshes[a[3].parse::<usize>().unwrap()];
    for v in a[4..].iter().map(|x| x.parse::<usize>().unwrap()) {
        let p = m.positions[v];
        let bones: Vec<String> = (0..8)
            .filter(|&k| m.weights[v][k] > 0)
            .map(|k| {
                let j = m.joints[v][k] as usize;
                let n = m.palette.get(j).map_or("?".to_string(), |h| names.get(h).map_or(format!("{h:08x}"), |s| s.to_string()));
                format!("{n}:{}", m.weights[v][k])
            })
            .collect();
        println!("{v} ({:.3} {:.3} {:.3}) {}", p[0], p[1], p[2], bones.join(" "));
    }
}

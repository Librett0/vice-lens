//! Decode a drawable to OBJ and print what it references.
//!
//! `model <archive> <offset-hex> <out.obj>`

use std::io::Write;

use rage::{Archive, drawable};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let archive = Archive::open(&a[0]).unwrap();
    let off = u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap();
    let res = archive.resource_at(off).unwrap();
    let d = drawable::parse(&res.virt, &res.phys).expect("not a drawable");
    let tris: usize = d.meshes.iter().map(|m| m.indices.len() / 3).sum();
    println!("{} sphere {:?} min {:?} max {:?}", d.name, d.sphere, d.min, d.max);
    println!("{} meshes, {tris} triangles, {} shaders, {} embedded textures", d.meshes.len(), d.shaders.len(), d.embedded.len());
    for (i, s) in d.shaders.iter().enumerate() {
        let emb = match s.first() {
            Some(drawable::TexRef::Embedded(at)) => d.embedded.iter().find(|t| t.at == *at).map(|t| t.name.clone()),
            _ => None,
        };
        println!("  shader {i}: {s:x?} {}", emb.unwrap_or_default());
    }
    for (i, m) in d.meshes.iter().enumerate() {
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for p in &m.positions {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        println!("  mesh {i}: shader {} {} verts {} tris, bounds {lo:?}..{hi:?}", m.shader, m.positions.len(), m.indices.len() / 3);
    }
    let Some(out) = a.get(2) else { return };
    let mut f = std::io::BufWriter::new(std::fs::File::create(out).unwrap());
    let mut base = 1;
    for (i, m) in d.meshes.iter().enumerate() {
        writeln!(f, "g mesh_{i}_shader_{}", m.shader).unwrap();
        for p in &m.positions {
            writeln!(f, "v {} {} {}", p[0], p[2], -p[1]).unwrap();
        }
        for t in m.indices.chunks(3) {
            writeln!(f, "f {} {} {}", base + t[0], base + t[1], base + t[2]).unwrap();
        }
        base += m.positions.len() as u32;
    }
}

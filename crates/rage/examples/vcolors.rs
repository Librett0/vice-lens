//! Vertex colours (type 0x26 streams, RGBA8) of a drawable's meshes: per
//! mesh its shader and the spread of each channel, to see what the terrain
//! blend shader keeps there.
//!
//! `vcolors <archive> <offset-hex>`

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = rage::Archive::open(&a[0]).unwrap();
    let off = u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap();
    let r = ar.resource_at(off).unwrap();
    let d = rage::drawable::parse(&r.virt, &r.phys).unwrap();
    for (k, m) in d.meshes.iter().enumerate() {
        let Some(&(_, o)) = m.streams.iter().find(|s| s.0 == 0x26) else { continue };
        let n = m.positions.len();
        let px: Vec<[u8; 4]> = (0..n)
            .map(|i| {
                let b = m.vb + o + i * 4;
                [r.phys[b], r.phys[b + 1], r.phys[b + 2], r.phys[b + 3]]
            })
            .collect();
        let stat = |c: usize| {
            let mut v: Vec<u8> = px.iter().map(|p| p[c]).collect();
            v.sort();
            format!("{:3}/{:3}/{:3}", v[0], v[v.len() / 2], v[v.len() - 1])
        };
        let layered = d.layered.get(m.shader) == Some(&true);
        println!(
            "mesh {k:2} shader {:2} {} verts {n:4}  R {}  G {}  B {}  A {}  first {:?}",
            m.shader,
            if layered { "terrain" } else { "       " },
            stat(0),
            stat(1),
            stat(2),
            stat(3),
            &px[..px.len().min(4)]
        );
    }
}

//! Print each mesh's vertex-buffer header (0x00..0x80) next to its decoded
//! bounds, to find the field that orders the position axes.
use rage::{Archive, drawable, le};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    for off in &a[1..] {
        let off = u64::from_str_radix(off.trim_start_matches("0x"), 16).unwrap();
        let res = ar.resource_at(off).unwrap();
        let d = drawable::parse(&res.virt, &res.phys).unwrap();
        println!("{} min {:?} max {:?}", d.name, d.min, d.max);
        for (i, m) in d.meshes.iter().enumerate().take(4) {
            let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
            for p in &m.positions {
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
            println!("  mesh {i}: ext [{:.2} {:.2} {:.2}]", hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]);
            let p = &res.phys;
            for row in 0..8 {
                let o = m.vb + row * 16;
                let w: Vec<String> = (0..4).map(|k| format!("{:08x}", le::u32(p, o + k * 4))).collect();
                let f: Vec<String> = (0..4).map(|k| format!("{:>10.4}", f32::from_bits(le::u32(p, o + k * 4)))).collect();
                println!("    +{:02x} {}   {}", row * 16, w.join(" "), f.join(" "));
            }
        }
    }
}

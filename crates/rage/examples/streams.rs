//! Print a drawable mesh's vertex stream headers (all 16 slots) and the
//! first vertices of each stream as raw bytes.
//!
//! `streams <archive> <offset-hex> <mesh> [vertices]`
use rage::{Archive, drawable, le};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let off = u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap();
    let mi: usize = a[2].parse().unwrap();
    let nv: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(6);
    let res = ar.resource_at(off).unwrap();
    let d = drawable::parse(&res.virt, &res.phys).unwrap();
    let m = &d.meshes[mi];
    let p = &res.phys;
    println!("{} mesh {mi}: shader {} verts {}", d.name, m.shader, m.positions.len());
    let heads: Vec<(usize, u32)> = (0..16).map(|k| (k, le::u32(p, m.vb + k * 4))).filter(|&(_, h)| h != 0).collect();
    let mut offs: Vec<usize> = heads.iter().map(|&(_, h)| ((h >> 8) << 1) as usize).collect();
    offs.sort();
    for &(k, h) in &heads {
        let o = ((h >> 8) << 1) as usize;
        let next = offs.iter().copied().find(|&x| x > o).unwrap_or(le::u32(p, m.vb + 0x58) as usize);
        let stride = (next - o) / m.positions.len().max(1);
        let mut line = format!("  slot {k:>2} type {:#04x} @{o:#x} ~{stride} B/vertex:", h & 0xff);
        for i in 0..nv {
            let s = stride.clamp(1, 16);
            let at = m.vb + o + i * s;
            let bytes: Vec<String> = p[at..at + s].iter().map(|b| format!("{b:02x}")).collect();
            line += &format!(" {}", bytes.join(""));
        }
        println!("{line}");
    }
}

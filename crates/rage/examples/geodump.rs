//! A drawable's LOD 0 geometries raw, to find what ties skinned vertices
//! to bones: each geometry struct as words, and every pointer in it read as
//! a u16 array (the first values, and the bone names they would index).
//!
//! `geodump <archive> <offset> [geometries]`
use rage::{Archive, drawable, le, rsc::vptr};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let r = ar.resource_at(u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap()).unwrap();
    let v = &r.virt;
    let b = drawable::base(v).unwrap();
    let bones = drawable::bones(v, b);
    let want: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(2);
    let l = vptr(le::u64(v, b + 0x50)).unwrap();
    let models = vptr(le::u64(v, l)).unwrap();
    let n = le::u16(v, l + 8) as usize;
    let mut shown = 0;
    for mi in 0..n {
        let m = vptr(le::u64(v, models + mi * 8)).unwrap();
        println!("model {mi} at {m:#x}");
        for row in 0..4 {
            let w: Vec<String> = (0..4).map(|k| format!("{:016x}", le::u64(v, m + row * 32 + k * 8))).collect();
            println!("   +{:02x} {}", row * 32, w.join(" "));
        }
        let ga = vptr(le::u64(v, m + 8)).unwrap();
        for gi in 0..le::u16(v, m + 0x10) as usize {
            if shown >= want {
                return;
            }
            shown += 1;
            let g = vptr(le::u64(v, ga + gi * 8)).unwrap();
            println!(" geometry {gi} at {g:#x}");
            for row in 0..6 {
                let o = g + row * 32;
                let w: Vec<String> = (0..4).map(|k| format!("{:016x}", le::u64(v, o + k * 8))).collect();
                println!("   +{:03x} {}", row * 32, w.join(" "));
            }
            for f in (0..0xc0).step_by(8) {
                let Some(p) = vptr(le::u64(v, g + f)) else { continue };
                let vals: Vec<u16> = (0..16).map(|k| le::u16(v, p + k * 2)).collect();
                let names: Vec<&str> = vals.iter().take(8).map(|&i| bones.get(i as usize).map_or("-", |b| b.name.as_str())).collect();
                println!("   ptr +{f:#x} -> {p:#x}: {vals:?} {names:?}");
            }
        }
    }
}

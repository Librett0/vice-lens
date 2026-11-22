//! A fragment's structure, for vehicles: its drawable's LOD 0 models (each
//! with geometry count and the words at +0x28 that bind it to a bone), the
//! skeleton pointer, and the bone-like names in the resource.
//!
//! `fragdump <archive> <offset> [words]`
use rage::{Archive, drawable, le, rsc::vptr};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let r = ar.resource_at(u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap()).unwrap();
    let v = &r.virt;
    println!("virt {} phys {}", v.len(), r.phys.len());
    for row in 0..16 {
        let w: Vec<String> = (0..4).map(|k| format!("{:016x}", le::u64(v, row * 32 + k * 8))).collect();
        println!("  +{:03x} {}", row * 32, w.join(" "));
    }
    let b = drawable::base(v).unwrap();
    println!("drawable at {b:#x}, name {:?}, skeleton ptr {:#x}", drawable::name(v, b), le::u64(v, b + 0x18));
    let l = vptr(le::u64(v, b + 0x50)).unwrap();
    let models = vptr(le::u64(v, l)).unwrap();
    let n = le::u16(v, l + 8) as usize;
    for mi in 0..n {
        let m = vptr(le::u64(v, models + mi * 8)).unwrap();
        let gn = le::u16(v, m + 0x10);
        let words: Vec<String> = (0..6).map(|k| format!("{:08x}", le::u32(v, m + 0x20 + k * 4))).collect();
        println!("  model {mi}: {gn} geometries, +0x20.. {}", words.join(" "));
    }
    if let Some(sk) = vptr(le::u64(v, b + 0x18)) {
        for row in 0..6 {
            let w: Vec<String> = (0..4).map(|k| format!("{:016x}", le::u64(v, sk + row * 32 + k * 8))).collect();
            println!("  skel +{:02x} {}", row * 32, w.join(" "));
        }
    }
    // Printable names with a part-like word in them.
    let want = a
        .get(2)
        .map(|s| s.split(',').map(str::to_string).collect::<Vec<_>>())
        .unwrap_or_else(|| ["door", "wheel", "bonnet", "boot", "chassis", "window", "seat", "steer", "bodyshell", "misc_", "wing", "bumper"].map(String::from).to_vec());
    let mut i = 0;
    let mut shown = 0;
    while i < v.len() && shown < 120 {
        let s = le::cstr(v, i);
        if s.len() >= 3 && s.iter().all(|c| c.is_ascii_graphic()) {
            let t = String::from_utf8_lossy(s);
            if want.iter().any(|w| t.contains(w.as_str())) {
                print!("{i:#x}:{t} ");
                shown += 1;
            }
            i += s.len() + 1;
        } else {
            i += 1;
        }
    }
    println!();
}

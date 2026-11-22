//! Write a TTPF texture's whole allocation (still tiled, mips included) to
//! a file, with its size and format: material for `swzsearch`.
//!
//! `ttpfraw <game dir> <key hex> [out.bin]`

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let key = u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap();
    let mut stack = vec![std::path::PathBuf::from(&a[0])];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "ttpf") {
                let Ok(pk) = rage::ttpf::Pack::open(&p) else { continue };
                if let Some(i) = pk.find(key) {
                    let e = &pk.entries[i];
                    let raw = pk.read_virtual(e.offset, e.alloc as usize).unwrap();
                    if let Some(out) = a.get(2) {
                        std::fs::write(out, &raw).unwrap();
                    }
                    println!("{}x{} dxgi {} mips {} alloc {:#x} record {:08x?}", e.width, e.height, e.dxgi, e.mips, e.alloc, e.raw);
                    return;
                }
            }
        }
    }
}

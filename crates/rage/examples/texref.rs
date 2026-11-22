//! The texture references of a model's shaders, raw: the reference struct,
//! the 16 bytes it points at (pointer, TTPF key), and what that pointer
//! points at. `texref <archive> <offset> <name>`
use rage::{Archive, drawable, le, rsc::vptr};
fn hex(b: &[u8]) -> String {
    b.chunks(4).map(|c| c.iter().map(|x| format!("{x:02x}")).collect::<String>()).collect::<Vec<_>>().join(" ")
}
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let r = ar.resource_at(u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap()).unwrap();
    let v = &r.virt;
    let d = drawable::parse_model(v, &r.phys, rage::joaat(&a[2])).unwrap();
    let b = (0..le::u16(v, 0x38) as usize).find_map(|k| drawable::dictionary_entry(v, k).filter(|&b| drawable::name(v, b) == d.name)).or(drawable::base(v)).unwrap_or(0);
    let sg = vptr(le::u64(v, b + 0x10)).unwrap();
    let sa = vptr(le::u64(v, sg + 0x10)).unwrap();
    for si in 0..le::u16(v, sg + 0x18) as usize {
        let sh = vptr(le::u64(v, sa + si * 8)).unwrap();
        let refs = vptr(le::u64(v, sh + 0x10)).unwrap();
        let info = vptr(le::u64(v, sh + 0x20)).unwrap();
        let (nt, np) = (le::u8(v, info + 1) as usize, le::u8(v, info + 4) as usize);
        println!("shader {si}");
        for i in 0..np {
            let (hash, data) = (le::u32(v, info + 8 + i * 8), le::u32(v, info + 12 + i * 8));
            let ti = ((data >> 2) & 255) as usize;
            if data & 3 != 0 || ti >= nt {
                continue;
            }
            let tp = vptr(le::u64(v, refs + ti * 8)).unwrap();
            println!("  {hash:08x} ref {:#x}: {}", tp, hex(&v[tp..tp + 0x50]));
            if let Some(r16) = vptr(le::u64(v, tp + 0x30)) {
                let p = le::u64(v, r16);
                println!("     -> {}  ptr {p:#x}", hex(&v[r16..r16 + 16]));
                if let Some(t) = vptr(p) {
                    let s = &v[t..(t + 0x40).min(v.len())];
                    println!("     ptr -> {}  {:?}", hex(s), String::from_utf8_lossy(le::cstr(v, t)));
                }
            }
        }
    }
}

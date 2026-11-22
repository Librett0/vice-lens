//! An animation's block pointers as stored (u64), and the resource's
//! virtual and physical segment sizes.
//!
//! `blockptrs <archive> <dictionary offset> <animation virt offset>...`
use rage::{Archive, le, rsc::vptr};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let hex = |s: &str| u64::from_str_radix(s.trim_start_matches("0x"), 16).unwrap() as usize;
    let r = ar.resource_at(hex(&a[1]) as u64).unwrap();
    let v = &r.virt;
    println!("virtual {:#x}, physical {:#x}", v.len(), r.phys.len());
    for x in &a[2..] {
        let an = hex(x);
        let list = vptr(le::u64(v, an + 0x50)).unwrap();
        let n = le::u16(v, an + 0x58) as usize;
        let ptrs: Vec<String> = (0..n).map(|k| format!("{:#x}", le::u64(v, list + k * 8))).collect();
        println!("{x}: list {:#x} ({}), {n} blocks: {}", le::u64(v, an + 0x50), list, ptrs.join(" "));
    }
}

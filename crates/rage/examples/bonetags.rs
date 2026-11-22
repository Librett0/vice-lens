//! A skeleton's bone records: index, name, and the u16 fields at +0x40..
//! +0x50 (sibling, parent, index, tag…), with the bones sorted by tag.
//!
//! `bonetags <archive> <offset> <name>`
use rage::{Archive, drawable, le, rsc::vptr};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let r = ar.resource_at(u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap()).unwrap();
    let v = &r.virt;
    let b = drawable::base(v).unwrap();
    let sk = vptr(le::u64(v, b + 0x18)).unwrap();
    let n = le::u16(v, sk + 0x6c) as usize;
    let recs = vptr(le::u64(v, sk + 0x28)).unwrap();
    println!("skeleton at {sk:#x}: {}", (0..0x80).step_by(8).map(|k| format!("{:016x}", le::u64(v, sk + k))).collect::<Vec<_>>().join(" "));
    let mut rows = Vec::new();
    for i in 0..n {
        let rec = recs + i * 80;
        let name = vptr(le::u64(v, rec + 0x30)).map(|p| String::from_utf8_lossy(le::cstr(v, p)).into_owned()).unwrap_or_default();
        let f: Vec<u16> = (0..8).map(|k| le::u16(v, rec + 0x40 + k * 2)).collect();
        rows.push((f[5], i, name, f));
    }
    for (tag, i, name, f) in &rows {
        println!("{i:3} {name:32} tag {tag:5} fields {f:?}");
    }
}

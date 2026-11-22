//! A drawable's skeleton: bone records (80 bytes each, from the pointer at
//! skeleton +0x28) with any name a pointer in them leads to, and the
//! matrices at +0x30 and +0x38 for the bones asked for.
//!
//! `skeldump <archive> <offset> [bone name filter]`
use rage::{Archive, drawable, le, rsc::vptr};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let r = ar.resource_at(u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap()).unwrap();
    let v = &r.virt;
    let b = drawable::base(v).unwrap();
    let sk = vptr(le::u64(v, b + 0x18)).unwrap();
    let n = le::u16(v, sk + 0x6c) as usize;
    let bones = vptr(le::u64(v, sk + 0x28)).unwrap();
    let filt = a.get(2).cloned().unwrap_or_default();
    let f = |o: usize| f32::from_bits(le::u32(v, o));
    println!("{n} bones at {bones:#x}");
    // With COMPOSE, each bone's place found by walking its parents
    // (u16 at +0x46), the matrices at +0x30 taken as relative to the parent.
    if std::env::var_os("COMPOSE").is_some() {
        let mats = vptr(le::u64(v, sk + 0x30)).unwrap();
        let m = |i: usize| -> [f32; 16] { std::array::from_fn(|k| f(mats + i * 64 + k * 4)) };
        let parent = |i: usize| le::u16(v, bones + i * 80 + 0x46);
        let mul = |a: &[f32; 16], b: &[f32; 16]| -> [f32; 16] { std::array::from_fn(|k| (0..4).map(|j| a[(k / 4) * 4 + j] * b[j * 4 + k % 4]).sum()) };
        for i in 0..n {
            let name = vptr(le::u64(v, bones + i * 80 + 0x30)).map(|p| String::from_utf8_lossy(le::cstr(v, p)).into_owned()).unwrap_or_default();
            if !filt.is_empty() && !name.contains(&filt) {
                continue;
            }
            let mut g = m(i);
            let mut j = parent(i);
            let mut chain = vec![];
            while j != 0xffff && chain.len() < 40 {
                g = mul(&g, &m(j as usize));
                chain.push(j);
                j = parent(j as usize);
            }
            println!("[{i}] {name}: local {:.3?} global {:.3?} parents {chain:?}", [m(i)[12], m(i)[13], m(i)[14]], [g[12], g[13], g[14]]);
        }
        return;
    }
    for i in 0..n {
        let bo = bones + i * 80;
        let name = (0..10)
            .filter_map(|k| vptr(le::u64(v, bo + k * 8)))
            .map(|p| String::from_utf8_lossy(le::cstr(v, p)).into_owned())
            .find(|s| s.len() > 1 && s.chars().all(|c| c.is_ascii_graphic()))
            .unwrap_or_default();
        if !filt.is_empty() && !name.contains(&filt) && i > 3 {
            continue;
        }
        let words: Vec<String> = (0..20).map(|k| format!("{:08x}", le::u32(v, bo + k * 4))).collect();
        println!("[{i}] {name}\n     {}", words.join(" "));
        for (label, at) in [("m30", 0x30usize), ("m38", 0x38)] {
            if let Some(m) = vptr(le::u64(v, sk + at)) {
                let mo = m + i * 64;
                let row: Vec<String> = (0..16).map(|k| format!("{:.3}", f(mo + k * 4))).collect();
                println!("     {label} {}", row.join(" "));
            }
        }
    }
}

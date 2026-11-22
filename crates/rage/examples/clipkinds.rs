//! A clip dictionary's entries as stored: per bucket entry its key, the
//! clip's offset, the u32 at the clip's `+0x20` (its type), its name, and
//! the clip's first 0x90 bytes as u64 words.
//!
//! `clipkinds <archive> <dictionary offset>`
use rage::{Archive, le, rsc::vptr};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let v = ar.resource_at(u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap()).unwrap().virt;
    let map = vptr(le::u64(&v, 0x48)).unwrap();
    for b in 0..le::u16(&v, 0x40) as usize {
        let mut e = vptr(le::u64(&v, map + b * 8));
        while let Some(eo) = e.filter(|&eo| eo + 0x18 <= v.len()) {
            e = vptr(le::u64(&v, eo + 0x10));
            let Some(c) = vptr(le::u64(&v, eo + 8)).filter(|&c| c + 0x90 <= v.len()) else { continue };
            let name = vptr(le::u64(&v, c + 0x28)).filter(|&o| o < v.len()).map(|o| String::from_utf8_lossy(le::cstr(&v, o)).into_owned()).unwrap_or_default();
            let words: Vec<String> = (0..0x90 / 8).map(|k| format!("{:x}", le::u64(&v, c + k * 8))).collect();
            println!("key {:08x} clip {c:#x} type {} {:?}\n   {}", le::u32(&v, eo), le::u32(&v, c + 0x20), name.chars().take(40).collect::<String>(), words.join(" "));
        }
    }
}

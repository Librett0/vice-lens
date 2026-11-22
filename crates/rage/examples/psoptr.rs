//! Follow a PSO pointer (block number in bits 0..12, offset from bit 20) in
//! a map data resource and dump the bytes there as words.
//!
//! `psoptr <archive> <offset-hex> <pointer-hex> <bytes>`

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = rage::Archive::open(&a[0]).unwrap();
    let r = ar.resource_at(u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap()).unwrap();
    let p = u64::from_str_radix(a[2].trim_start_matches("0x"), 16).unwrap();
    let n: usize = a[3].parse().unwrap();
    let list = rage::rsc::vptr(rage::le::u64(&r.virt, 0x38)).unwrap();
    let k = (p & 0xfff) as usize - 1;
    let rec = list + k * 16;
    let (t, size, at) = (rage::le::u32(&r.virt, rec), rage::le::u32(&r.virt, rec + 4) as usize, rage::le::u64(&r.virt, rec + 8));
    let data = rage::rsc::vptr(at).map(|o| &r.virt[o..o + size]).or_else(|| rage::rsc::pptr(at).map(|o| &r.phys[o..o + size])).unwrap();
    let off = (p >> 20) as usize;
    println!("block {} type {t:08x} size {size}, at +{off:#x}", k + 1);
    for (i, row) in data[off..(off + n).min(size)].chunks(32).enumerate() {
        let w: Vec<String> = row.chunks(4).map(|c| format!("{:08x}", u32::from_le_bytes(c.try_into().unwrap_or([0; 4])))).collect();
        println!("  +{:#06x}: {}", i * 32, w.join(" "));
    }
}

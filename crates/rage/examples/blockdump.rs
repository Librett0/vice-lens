//! Dump the blocks of a `0DRPz` map data resource: type, size, and the
//! first bytes of each as words and floats; with block types given, those
//! blocks whole.
//!
//! `blockdump <archive> <offset-hex> [type-hex...]`

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = rage::Archive::open(&a[0]).unwrap();
    let r = ar.resource_at(u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap()).unwrap();
    let want: Vec<u32> = a[2..].iter().map(|t| u32::from_str_radix(t, 16).unwrap()).collect();
    for (t, b) in rage::mapdata::blocks(&r.virt, &r.phys) {
        let whole = want.contains(&t);
        println!("block {t:08x}: {} bytes", b.len());
        let n = if whole { b.len() } else { b.len().min(64) };
        for row in b[..n].chunks(32) {
            let w: Vec<String> = row
                .chunks(4)
                .map(|c| {
                    let v = u32::from_le_bytes(c.try_into().unwrap_or([0; 4]));
                    let f = f32::from_bits(v);
                    if f.is_finite() && f.abs() > 1e-3 && f.abs() < 1e6 { format!("{v:08x}({f:.3})") } else { format!("{v:08x}") }
                })
                .collect();
            println!("    {}", w.join(" "));
        }
    }
}

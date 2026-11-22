//! One block of a `0DRPz` resource, as words from a byte offset:
//! `drpzblock <archive> <offset> <type-hex> <from-hex> <bytes-hex> [nth]`
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = rage::Archive::open(&a[0]).unwrap();
    let hex = |s: &str| u64::from_str_radix(s.trim_start_matches("0x"), 16).unwrap();
    let r = ar.resource_at(hex(&a[1])).unwrap();
    let t = hex(&a[2]) as u32;
    let (from, n) = (hex(&a[3]) as usize, hex(&a[4]) as usize);
    let nth: usize = a.get(5).and_then(|s| s.parse().ok()).unwrap_or(0);
    let Some((_, b)) = rage::mapdata::blocks(&r.virt, &r.phys).into_iter().filter(|x| x.0 == t).nth(nth) else { return };
    println!("{} bytes", b.len());
    for o in (from..(from + n).min(b.len())).step_by(32) {
        let w: Vec<String> = (0..8)
            .filter(|k| o + 4 * k + 4 <= b.len())
            .map(|k| {
                let v = u32::from_le_bytes(b[o + 4 * k..o + 4 * k + 4].try_into().unwrap());
                let f = f32::from_bits(v);
                if f.is_finite() && f.abs() > 1e-3 && f.abs() < 1e5 { format!("{v:08x}({f:.3})") } else { format!("{v:08x}") }
            })
            .collect();
        println!("{o:6x} {}", w.join(" "));
    }
}

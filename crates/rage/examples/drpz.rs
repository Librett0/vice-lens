//! Block types of 0DRPz resources at the listed offsets.
//!
//! `drpz <archive> <offsets.txt>`
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = rage::Archive::open(&a[0]).unwrap();
    for line in std::fs::read_to_string(&a[1]).unwrap().lines() {
        let Some(off) = line.split_whitespace().next().and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok()) else { continue };
        let Ok(d) = ar.file_at(off) else { continue };
        if d.len() < 0x50 {
            continue;
        }
        let le32 = |o: usize| u32::from_le_bytes(d[o..o + 4].try_into().unwrap());
        let list = rage::le_u64(&d, 0x38).wrapping_sub(0x5000_0000) as usize;
        let n = u16::from_le_bytes([d[0x4c], d[0x4d]]) as usize;
        let mut types = Vec::new();
        for i in 0..n.min(10) {
            let r = list + 16 * i;
            if r + 16 > d.len() {
                break;
            }
            types.push(format!("{:08x}:{}", le32(r), le32(r + 4)));
        }
        println!("{off:#x}\t{}\t{n}\t{}", d.len(), types.join(","));
    }
}

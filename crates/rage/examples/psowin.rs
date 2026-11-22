//! A PSO file's blocks, and the bytes around an offset as words, with the
//! block that holds it: to see what a hash found by `findbytes` sits in.
//!
//! `psowin <archive> <file-offset> <byte-offset> [span]`
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = rage::Archive::open(&a[0]).unwrap();
    let hex = |s: &str| u64::from_str_radix(s.trim_start_matches("0x"), 16).unwrap();
    let d = ar.file_at(hex(&a[1])).unwrap();
    let at = hex(&a[2]) as usize;
    let span = a.get(3).map_or(0x100, |s| hex(s) as usize);
    let p = rage::pso::Pso::parse(d).unwrap();
    for (i, b) in p.blocks.iter().enumerate() {
        let mark = if (b.offset..b.offset + b.size).contains(&at) { "  <==" } else { "" };
        println!("block {:3} {:08x} at {:#9x} size {:9}{mark}", i + 1, b.type_hash, b.offset, b.size);
    }
    let b = p.blocks.iter().find(|b| (b.offset..b.offset + b.size).contains(&at));
    let base = b.map_or(0, |b| b.offset);
    let start = at.saturating_sub(span) & !15;
    for o in (start..(at + span).min(p.data.len())).step_by(16) {
        let w: Vec<String> = (0..4)
            .map(|k| {
                let v = p.u32(o + 4 * k);
                let f = f32::from_bits(v);
                if f.is_finite() && f.abs() > 1e-3 && f.abs() < 1e5 { format!("{v:08x}({f:.3})") } else { format!("{v:08x}") }
            })
            .collect();
        let s: String = p.data[o..o + 16].iter().map(|&c| if c.is_ascii_graphic() { c as char } else { '.' }).collect();
        println!("{:#8x} {} {s}", o - base, w.join(" "));
    }
}

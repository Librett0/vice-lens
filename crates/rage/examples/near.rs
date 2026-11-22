//! The files of an archive around an offset: offset, size, and what they
//! look like (resource name, magic bytes).
//!
//! `near <archive> <offset> <span>`
use rage::Archive;
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let at = u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap();
    let span = u64::from_str_radix(a[2].trim_start_matches("0x"), 16).unwrap();
    let mut streams = Vec::new();
    ar.scan(|s, _| {
        streams.push(s);
        true
    });
    for f in Archive::files(&streams) {
        let o = f.offset();
        if o + span < at || o > at + span {
            continue;
        }
        let Ok(d) = ar.read_file(&f) else { continue };
        let kind = rage::index::kind(&d);
        println!("{o:#x} {:8} {}", d.len(), kind.chars().take(110).collect::<String>());
    }
}

//! Streams of an archive that hold every one of some u32 values (name
//! hashes, little-endian): where a structure names two things together.
//!
//! `hashpair <archive> <hex u32>...`
use rage::Archive;
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let pats: Vec<[u8; 4]> = a[1..].iter().map(|h| u32::from_str_radix(h.trim_start_matches("0x"), 16).unwrap().to_le_bytes()).collect();
    let mut hits = 0;
    ar.scan(|s, d| {
        if pats.iter().all(|p| d.windows(4).any(|w| w == p)) {
            hits += 1;
            println!("stream {:#x} ({} bytes)", s.offset, d.len());
        }
        hits < 200
    });
}

//! Search every file in archives for strings and their joaat hashes.
//!
//! `grep <archive>... -- <needle>...`

use rage::{Archive, joaat};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let split = args.iter().position(|a| a == "--").expect("--");
    let (archives, needles) = (&args[..split], &args[split + 1..]);
    let mut pats: Vec<(String, Vec<u8>)> = Vec::new();
    for n in needles {
        pats.push((n.clone(), n.as_bytes().to_vec()));
        pats.push((format!("joaat({n})"), joaat(n).to_le_bytes().to_vec()));
        pats.push((format!("literal({n})"), literal(n).to_le_bytes().to_vec()));
        pats.push((n.to_lowercase(), n.to_lowercase().into_bytes()));
    }
    for path in archives {
        let a = Archive::open(path).expect("open");
        let mut streams = Vec::new();
        a.scan(|s, _| {
            streams.push(s);
            true
        });
        for f in Archive::files(&streams) {
            let Ok(d) = a.read_file(&f) else { continue };
            for (label, p) in &pats {
                if let Some(i) = memfind(&d, p) {
                    let head = String::from_utf8_lossy(&d[..d.len().min(8)]).replace(|c: char| !c.is_ascii_graphic(), ".");
                    println!("{path} {:#012x} size {:8} at {i:8} head {head:8} {label}", f.offset(), d.len());
                }
            }
        }
    }
}

/// joaat without lowercasing.
fn literal(s: &str) -> u32 {
    let mut h: u32 = 0;
    for &c in s.as_bytes() {
        h = h.wrapping_add(c as u32);
        h = h.wrapping_add(h << 10);
        h ^= h >> 6;
    }
    h = h.wrapping_add(h << 3);
    h ^= h >> 11;
    h.wrapping_add(h << 15)
}

fn memfind(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).position(|w| w == n)
}

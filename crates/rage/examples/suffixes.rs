//! Count `.#xx` name suffixes in the files at the listed offsets.
//!
//! `suffixes <archive> <offsets.txt>`

use std::collections::BTreeMap;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = rage::Archive::open(&a[0]).unwrap();
    let mut seen: BTreeMap<String, (usize, String, u64)> = BTreeMap::new();
    for line in std::fs::read_to_string(&a[1]).unwrap().lines() {
        let Some(off) = line.split_whitespace().next().and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok()) else { continue };
        let Ok(d) = ar.file_at(off) else { continue };
        for i in 1..d.len().saturating_sub(3) {
            if d[i] == b'#' && d[i - 1] == b'.' && d[i + 1].is_ascii_lowercase() && d[i + 2].is_ascii_lowercase() {
                let ext = String::from_utf8_lossy(&d[i - 1..i + 3]).into_owned();
                let mut s = i - 1;
                while s > 0 && (d[s - 1].is_ascii_alphanumeric() || d[s - 1] == b'_') {
                    s -= 1;
                }
                let e = seen.entry(ext).or_insert((0, String::from_utf8_lossy(&d[s..i + 3]).into_owned(), off));
                e.0 += 1;
            }
        }
    }
    for (ext, (n, example, off)) in seen {
        println!("{ext} {n:8} e.g. {example} in {off:#x}");
    }
}

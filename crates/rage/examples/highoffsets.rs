//! TTPF packs whose virtual stream passes 4 GiB: their size (header and
//! chunk table), and how many textures have bit 20 of the offset word set
//! (their data past 4 GiB).
//!
//! `highoffsets <game dir>`

fn main() {
    let dir = std::env::args().nth(1).unwrap();
    let mut stack = vec![std::path::PathBuf::from(dir)];
    let (mut total_tex, mut high_tex) = (0usize, 0usize);
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if !p.extension().is_some_and(|x| x == "ttpf") {
                continue;
            }
            let Ok(pk) = rage::ttpf::Pack::open(&p) else { continue };
            let high = pk.entries.iter().filter(|e| e.raw[2] & (1 << 20) != 0).count();
            total_tex += pk.entries.len();
            high_tex += high;
            let end = pk.chunks_over(0, u64::MAX).last().map_or(0, |c| c.2);
            if high > 0 || end > 1 << 32 {
                println!("{}: {} textures, {high} with bit 20; chunks end at {end:#x}", p.file_name().unwrap().to_string_lossy(), pk.entries.len());
            }
        }
    }
    println!("{high_tex} of {total_tex} textures have bit 20 set");
}

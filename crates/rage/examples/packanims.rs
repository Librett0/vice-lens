//! The clip dictionaries of other archives (content packs): per archive,
//! `anim::survey`'s rows as `archive<TAB>offset<TAB>name<TAB>clips<TAB>whole`.
//!
//! `packanims <archive>...`
use rage::{Archive, anim};
fn main() {
    for path in std::env::args().skip(1) {
        let Ok(a) = Archive::open(&path) else {
            eprintln!("{path}: not an archive");
            continue;
        };
        let rows = anim::survey(&a, &|k, n| {
            if k % 20480 == 0 {
                eprintln!("{path}: {k} of {n}");
            }
        });
        eprintln!("{path}: {} dictionaries", rows.len());
        for (o, name, clips, whole) in rows {
            println!("{path}\t{o:#x}\t{name}\t{clips}\t{}", whole as u8);
        }
    }
}

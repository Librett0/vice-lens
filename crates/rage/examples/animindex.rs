//! The animation list the app keeps (`anim-dicts.tsv`, as Setup makes it):
//! per clip dictionary its offset, name, clips, and whether all of it
//! decodes. `animindex <anim.rpf> > anim-dicts.tsv`
fn main() {
    let a = rage::Archive::open(std::env::args().nth(1).unwrap()).unwrap();
    for (o, n, c, ok) in rage::anim::survey(&a, &|d, t| {
        if d % 8192 == 0 {
            eprintln!("{d}/{t}");
        }
    }) {
        println!("{o:#x}\t{n}\t{c}\t{}", ok as u8);
    }
}

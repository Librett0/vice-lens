//! Print one label from every text table, to tell languages apart.
use rage::{Archive, text};
fn main() {
    let path = std::env::args().nth(1).expect("archive");
    let label = std::env::args().nth(2).unwrap_or_else(|| "PM_INDX_SET".into());
    let a = Archive::open(&path).unwrap();
    let mut streams = Vec::new();
    a.scan(|s, _| {
        streams.push(s);
        true
    });
    for f in Archive::files(&streams) {
        let Ok(d) = a.read_file(&f) else { continue };
        if !text::is_text_table(&d) {
            continue;
        }
        let Ok(t) = text::parse(&d) else { continue };
        if let Some(s) = t.get(&label) {
            println!("{:#012x} {:8} labels {:6} {s}", f.offset(), d.len(), t.len());
        }
    }
}

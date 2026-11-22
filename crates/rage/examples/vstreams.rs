//! A model's vertex streams: per mesh, each stream's type, start, and the
//! bytes it holds per vertex, with the first vertices' bytes, to tell
//! normals, colours and bone indices apart.
//!
//! `vstreams <archive> <offset> <name> [meshes]`
use rage::{Archive, drawable};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let r = ar.resource_at(u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap()).unwrap();
    let d = drawable::parse_model(&r.virt, &r.phys, rage::joaat(&a[2])).unwrap();
    let p = &r.phys;
    let n: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(3);
    for (mi, m) in d.meshes.iter().enumerate().take(n) {
        let nv = m.positions.len();
        let mut s = m.streams.clone();
        s.sort_by_key(|x| x.1);
        println!("mesh {mi}: shader {} verts {nv}", m.shader);
        // The 0x26 streams at spread-out vertices, as u16 pairs.
        if std::env::var_os("SPREAD").is_some() {
            for (k, &(t, o)) in s.iter().enumerate().filter(|x| x.1.0 == 0x26) {
                let end = s.get(k + 1).map_or(o + nv * 4, |x| x.1);
                let st = ((end - o) as f32 / nv.max(1) as f32).round() as usize;
                let rows: Vec<String> = (0..12)
                    .map(|j| {
                        let v = j * nv / 12;
                        let at = m.vb + o + v * st;
                        let pairs: Vec<String> = (0..st / 4).map(|q| format!("{}:{:04x}", rage::le::u16(p, at + q * 4), rage::le::u16(p, at + q * 4 + 2))).collect();
                        pairs.join(",")
                    })
                    .collect();
                println!("   0x26 at {o:#x} ({st} B): {}", rows.join(" | "));
            }
            continue;
        }
        for (k, &(t, o)) in s.iter().enumerate() {
            let end = s.get(k + 1).map_or(o + nv * 4, |x| x.1);
            let per = (end - o) as f32 / nv.max(1) as f32;
            let first: Vec<String> = (0..3)
                .map(|v| {
                    let st = per.round() as usize;
                    let at = m.vb + o + v * st;
                    p.get(at..at + st.min(16)).map(|b| b.iter().map(|x| format!("{x:02x}")).collect::<String>()).unwrap_or_default()
                })
                .collect();
            println!("   type {t:#04x} at {o:#x} {per:.2} B/vertex  {}", first.join(" "));
        }
    }
}

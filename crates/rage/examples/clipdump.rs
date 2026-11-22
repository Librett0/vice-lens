//! A clip decoded: per track of each animation it plays, its bone, kind,
//! format and values at a few times, to check what the channels hold.
//!
//! `clipdump <archive> <dictionary offset> <clip name part>`
use rage::{Archive, anim};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let r = ar.resource_at(u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap()).unwrap();
    let v = &r.virt;
    let (name, clips) = anim::dictionary(v).unwrap();
    println!("{name}: {} clips", clips.len());
    let c = clips.iter().find(|c| c.name.contains(&a[2])).unwrap_or(&clips[0]);
    println!("clip {} {:.3} s", c.name, c.length);
    for p in &c.parts {
        let an = anim::animation(v, p.animation).unwrap();
        println!(" {} {}..{} rate {} | {} frames, {} a block, {:.2} s, {} tracks, {} blocks", an.name, p.start, p.end, p.rate, an.frames, an.per_block, an.length, an.tracks.len(), an.blocks.len());
        let b = anim::block(v, an.blocks[0], an.tracks.len()).unwrap();
        println!("  block 0: {} frames, unknown channels {}", b.frames, b.unknown);
        for (i, t) in an.tracks.iter().enumerate().take(std::env::var("ALL").map_or(40, |_| 999)) {
            let vals: Vec<String> = [0, b.frames / 2, b.frames.saturating_sub(1)]
                .iter()
                .map(|&f| match b.value(i, f) {
                    Some(anim::Value::Rotation(q)) => format!("R({:.3},{:.3},{:.3},{:.3})", q[0], q[1], q[2], q[3]),
                    Some(anim::Value::Values(x, n)) => format!("V{:?}", x[..n].iter().map(|v| (v * 1000.0).round() / 1000.0).collect::<Vec<_>>()),
                    None => "-".into(),
                })
                .collect();
            println!("   {:3} bone {:08x} kind {:08x}/{} {}", i, t.bone, t.kind, t.format, vals.join(" "));
        }
    }
}

//! A track's values at every frame of an animation (types 0–8):
//! `trackvalues <archive> <dictionary offset> <animation virt offset> <track>...`
use rage::{Archive, anim};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let hex = |s: &str| u64::from_str_radix(s.trim_start_matches("0x"), 16).unwrap();
    let v = ar.resource_at(hex(&a[1])).unwrap().virt;
    let an = anim::animation(&v, hex(&a[2]) as usize).unwrap();
    let blocks: Vec<_> = an.blocks.iter().map(|&b| anim::block(&v, b, an.tracks.len()).unwrap()).collect();
    for t in &a[3..] {
        let ti: usize = t.parse().unwrap();
        let vals: Vec<String> = (0..an.frames)
            .map(|f| {
                let k = (f / an.per_block).min(blocks.len() - 1);
                match blocks[k].value(ti, f - k * an.per_block) {
                    Some(anim::Value::Rotation(q)) => format!("[{:.5},{:.5},{:.5},{:.5}]", q[0], q[1], q[2], q[3]),
                    Some(anim::Value::Values(x, n)) => format!("{:?}", &x[..n]),
                    None => "null".into(),
                }
            })
            .collect();
        println!("{ti} [{}]", vals.join(","));
    }
}

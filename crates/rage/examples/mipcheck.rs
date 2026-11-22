//! Check that lower mips are read from the right place: for plain
//! mipmapped BC1 textures, decode mip 1..3 and the top level shrunk by
//! box filtering, and print their mean difference per channel (small when
//! the mip is where `Pack::mip` reads it, large when it is not).
//!
//! `mipcheck <game dir> [textures to try]`

fn bc1(data: &[u8], w: u32, h: u32) -> Vec<[f32; 3]> {
    let (bw, bh) = (w.div_ceil(4), h.div_ceil(4));
    let mut out = vec![[0f32; 3]; (w * h) as usize];
    for by in 0..bh {
        for bx in 0..bw {
            let b = &data[((by * bw + bx) * 8) as usize..][..8];
            let c = |v: u16| [((v >> 11) & 31) as f32 * 255.0 / 31.0, ((v >> 5) & 63) as f32 * 255.0 / 63.0, (v & 31) as f32 * 255.0 / 31.0];
            let (v0, v1) = (u16::from_le_bytes([b[0], b[1]]), u16::from_le_bytes([b[2], b[3]]));
            let (c0, c1) = (c(v0), c(v1));
            let mix = |a: [f32; 3], b: [f32; 3], t: f32| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
            let pal = if v0 > v1 { [c0, c1, mix(c0, c1, 1.0 / 3.0), mix(c0, c1, 2.0 / 3.0)] } else { [c0, c1, mix(c0, c1, 0.5), [0.0; 3]] };
            let bits = u32::from_le_bytes([b[4], b[5], b[6], b[7]]);
            for k in 0..16 {
                let (x, y) = (bx * 4 + k % 4, by * 4 + k / 4);
                if x < w && y < h {
                    out[(y * w + x) as usize] = pal[((bits >> (k * 2)) & 3) as usize];
                }
            }
        }
    }
    out
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let want: usize = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(12);
    let mut paths: Vec<_> = std::fs::read_dir(std::path::Path::new(&a[0]).join("textures")).map(|r| r.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    if paths.is_empty() {
        let mut stack = vec![std::path::PathBuf::from(&a[0])];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p)
                } else if p.extension().is_some_and(|x| x == "ttpf") {
                    paths.push(p)
                }
            }
        }
    }
    paths.sort();
    let mut done = 0;
    'packs: for p in paths.iter().filter(|p| p.extension().is_some_and(|x| x == "ttpf")).step_by(5) {
        let Ok(pk) = rage::ttpf::Pack::open(p) else { continue };
        for (i, e) in pk.entries.iter().enumerate().step_by(997) {
            if !(e.mips && matches!(e.dxgi, 71 | 72) && e.width == e.height && (256..=2048).contains(&e.width) && pk.plain(i)) {
                continue;
            }
            let Ok(top) = pk.top_mip(i) else { continue };
            let top = bc1(&top, e.width, e.height);
            let mut line = format!("{} {:016x} {}x{}:", p.file_name().unwrap().to_string_lossy(), e.key, e.width, e.height);
            for level in 1..4u32 {
                let Ok((w, h, m)) = pk.mip(i, level) else {
                    line += " err";
                    continue;
                };
                let m = bc1(&m, w, h);
                let k = 1u32 << level;
                let mut diff = 0f64;
                for y in 0..h {
                    for x in 0..w {
                        let mut s = [0f32; 3];
                        for dy in 0..k {
                            for dx in 0..k {
                                let t = top[((y * k + dy) * e.width + x * k + dx) as usize];
                                for c in 0..3 {
                                    s[c] += t[c];
                                }
                            }
                        }
                        let n = (k * k) as f32;
                        let v = m[(y * w + x) as usize];
                        diff += (0..3).map(|c| (s[c] / n - v[c]).abs() as f64).sum::<f64>() / 3.0;
                    }
                }
                line += &format!(" L{level} {:5.1}", diff / (w * h) as f64);
            }
            println!("{line}");
            done += 1;
            if done >= want {
                break 'packs;
            }
        }
    }
}

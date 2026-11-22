//! Print UV ranges of a drawable's meshes, read as u16/8192 and as f16.
use rage::{Archive, drawable};
fn f16(b: u16) -> f32 {
    let s = if b >> 15 == 1 { -1.0 } else { 1.0 };
    let e = ((b >> 10) & 31) as i32;
    let m = (b & 1023) as f32;
    if e == 0 {
        s * m / 1024.0 * 2f32.powi(-14)
    } else if e == 31 {
        f32::NAN
    } else {
        s * (1.0 + m / 1024.0) * 2f32.powi(e - 15)
    }
}
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let off = u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap();
    let res = ar.resource_at(off).unwrap();
    let d = drawable::parse(&res.virt, &res.phys).unwrap();
    for (i, m) in d.meshes.iter().take(6).enumerate() {
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        let (mut hlo, mut hhi) = ([f32::MAX; 2], [f32::MIN; 2]);
        for uv in &m.uvs {
            for k in 0..2 {
                lo[k] = lo[k].min(uv[k]);
                hi[k] = hi[k].max(uv[k]);
                let raw = (uv[k] * 8192.0) as u16;
                let h = f16(raw);
                hlo[k] = hlo[k].min(h);
                hhi[k] = hhi[k].max(h);
            }
        }
        for &(t, o) in &m.streams {
            let at = m.vb + o;
            let p = &res.phys;
            let f = |k: usize| f32::from_le_bytes(p[at + k * 4..at + k * 4 + 4].try_into().unwrap());
            let h = |k: usize| u16::from_le_bytes(p[at + k * 2..at + k * 2 + 2].try_into().unwrap());
            println!("   type {t:#x} @{o:#x}: f32 {:?}  u16 {:?}  f16 {:?}", (0..4).map(f).collect::<Vec<_>>(), (0..8).map(h).collect::<Vec<_>>(), (0..4).map(|k| f16(h(k))).collect::<Vec<_>>());
        }
        println!("mesh {i} streams {:x?} shader {} verts {} u16/8192 {lo:?}..{hi:?}  f16 {hlo:?}..{hhi:?}", m.streams, m.shader, m.uvs.len());
    }
}

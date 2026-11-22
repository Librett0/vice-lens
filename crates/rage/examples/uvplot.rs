//! A drawable drawn flat, to see where a part of the mesh lies in texture
//! space: every mesh's triangles in UV0 (and UV1 when it has one), and the
//! mesh seen from above, each pixel coloured by height (blue low, red
//! high). `uvplot <archive> <offset> <name> <out-prefix>`
use rage::{Archive, drawable};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform};

fn tri(pm: &mut Pixmap, p: [[f32; 2]; 3], c: [u8; 3]) {
    let mut pb = PathBuilder::new();
    pb.move_to(p[0][0], p[0][1]);
    pb.line_to(p[1][0], p[1][1]);
    pb.line_to(p[2][0], p[2][1]);
    pb.close();
    let Some(path) = pb.finish() else { return };
    let mut paint = Paint::default();
    paint.set_color_rgba8(c[0], c[1], c[2], 255);
    paint.anti_alias = false;
    pm.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ar = Archive::open(&a[0]).unwrap();
    let r = ar.resource_at(u64::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap()).unwrap();
    let d = drawable::parse_model(&r.virt, &r.phys, rage::joaat(&a[2])).unwrap();
    let zs = d.meshes.iter().flat_map(|m| m.positions.iter().map(|p| p[2]));
    let (zmin, zmax) = zs.fold((f32::MAX, f32::MIN), |a, z| (a.0.min(z), a.1.max(z)));
    println!("z {zmin} .. {zmax}");
    let heat = |z: f32| {
        let t = ((z - zmin) / (zmax - zmin).max(1e-6)).clamp(0.0, 1.0);
        [(255.0 * t) as u8, (255.0 * (1.0 - (2.0 * t - 1.0).abs())) as u8, (255.0 * (1.0 - t)) as u8]
    };
    let s = 512.0;
    for (which, label) in [(0, "uv0"), (1, "uv1")] {
        let mut pm = Pixmap::new(512, 512).unwrap();
        pm.fill(tiny_skia::Color::from_rgba8(0, 0, 0, 255));
        let mut any = false;
        for m in &d.meshes {
            let uv = if which == 0 { &m.uvs } else { &m.uvs1 };
            if uv.len() != m.positions.len() {
                continue;
            }
            any = true;
            for t in m.indices.chunks(3) {
                let p = [0, 1, 2].map(|k| [uv[t[k] as usize][0] * s, uv[t[k] as usize][1] * s]);
                let z = (m.positions[t[0] as usize][2] + m.positions[t[1] as usize][2] + m.positions[t[2] as usize][2]) / 3.0;
                tri(&mut pm, p, heat(z));
            }
        }
        if any {
            pm.save_png(format!("{}-{label}.png", a[3])).unwrap();
        }
    }
    // From above: x right, y up the image (the model's front is +y).
    let (w, h) = (d.max[0] - d.min[0], d.max[1] - d.min[1]);
    let k = s / w.max(h);
    let mut pm = Pixmap::new(512, 512).unwrap();
    pm.fill(tiny_skia::Color::from_rgba8(0, 0, 0, 255));
    let mut all: Vec<([[f32; 2]; 3], f32)> = Vec::new();
    for m in &d.meshes {
        for t in m.indices.chunks(3) {
            let v = [0, 1, 2].map(|q| m.positions[t[q] as usize]);
            let p = v.map(|v| [(v[0] - d.min[0]) * k, s - (v[1] - d.min[1]) * k]);
            all.push((p, (v[0][2] + v[1][2] + v[2][2]) / 3.0));
        }
    }
    // Lowest first, so the top of the head is drawn last.
    all.sort_by(|a, b| a.1.total_cmp(&b.1));
    for (p, z) in all {
        tri(&mut pm, p, heat(z));
    }
    pm.save_png(format!("{}-top.png", a[3])).unwrap();
    println!("min {:?} max {:?}", d.min, d.max);
}

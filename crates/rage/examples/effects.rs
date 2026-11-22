//! Survey the shader effects of the world's models: per effect hash (the
//! u32 at +0 of a shader struct), how many shaders and how much surface use
//! it, how often in models named like decals, whether its albedo map
//! decodes, and a known name when one of the candidates hashes to it.
//!
//! `effects <game dir> <models.tsv> <step>`: every `step`th row. With
//! `GREY` set, it also lists each surface over 2000 m² whose albedo does not
//! decode (none found, or BCPack), with its effect and bucket.

use std::collections::HashMap;

use rage::rsc::vptr;
use rage::{Archive, drawable, le};

const NAMES: &str = "default normal normal_spec normal_spec_decal normal_decal decal decal_dirt decal_normal_only decal_tnt decal_spec_only
decal_emissive_only decal_emissive decal_shadow_only decal_glue decal_amb_only normal_spec_decal_detail normal_spec_decal_nopuddle
normal_spec_decal_tnt normal_decal_tnt normal_decal_pxm normal_decal_pxm_tnt normal_spec_dpm glass glass_pv glass_pv_env glass_env
glass_spec glass_normal_spec_reflect glass_emissive glass_emissivenight glass_breakable glass_displacement emissive emissive_alpha
emissivestrong emissivenight emissive_clip emissive_speclum spec spec_decal normal_spec_detail normal_spec_reflect normal_spec_cutout
cutout_fence cutout_fence_normal trees trees_lod trees_lod2 trees_normal trees_normal_spec grass grass_batch grass_fur terrain_cb_w_4lyr
terrain_cb_w_4lyr_lod terrain_cb_w_4lyr_cm terrain_cb_w_4lyr_cm_tnt terrain_cb_w_4lyr_2tex terrain_cb_w_4lyr_2tex_blend terrain_cb_4lyr
water_river water_fountain water_terrainfoam water_poolenv water_shallow water_decal water mirror_default mirror_decal mirror_crack
parallax ped vehicle_paint1 vehicle_paint2 vehicle_paint3 vehicle_basic vehicle_mesh vehicle_glass vehicle_decal vehicle_lightsemissive
alpha normal_alpha normal_spec_alpha spec_alpha cloth_default cutout normal_cutout decal_alpha lod lod_tree terrain ground road
road_decal puddle emissive_additive_alpha distance_map billboard sky clouds glass_decal normal_spec_wrinkle normal_detail
normal_spec_detail_dpm normal_spec_pxm normal_pxm normal_tnt normal_spec_tnt cable default_detail default_spec default_tnt
blend_2lyr normal_blend normal_spec_blend wet ptfx_model hair skin eye";

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let game = std::path::Path::new(&a[0]);
    let step: usize = a[2].parse().unwrap();
    let mut known: HashMap<u32, String> = HashMap::new();
    for n in NAMES.split_whitespace() {
        known.insert(rage::joaat(n), n.into());
        known.insert(rage::joaat(&format!("{n}.sps")), format!("{n}.sps"));
    }
    // Texture keys → decodable.
    let mut packs = Vec::new();
    let mut stack = vec![game.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "ttpf")
                && let Ok(pk) = rage::ttpf::Pack::open(&p)
            {
                packs.push(pk);
            }
        }
    }
    let decodable = |key: u64| packs.iter().find_map(|p| p.find(key).map(|i| p.plain(i)));
    let mut archives: HashMap<String, Archive> = HashMap::new();
    // effect → (shaders, area, in decal-named models, albedo none / bcpack / ok, examples)
    let mut stats: HashMap<u32, (usize, f64, usize, [f64; 3], Vec<String>)> = HashMap::new();
    let rows = std::fs::read_to_string(&a[1]).unwrap();
    for line in rows.lines().step_by(step) {
        let f: Vec<&str> = line.split('\t').collect();
        let (arch, off, name) = if f.len() == 2 { ("levels.rpf", f[0], f[1]) } else { (f[0], f[1], f[2]) };
        let ar = archives.entry(arch.into()).or_insert_with(|| Archive::open(game.join(arch)).unwrap());
        let off = u64::from_str_radix(off.trim_start_matches("0x"), 16).unwrap();
        let Ok(r) = ar.resource_at(off) else { continue };
        let v = &r.virt;
        let stem = name.trim_end_matches(".#dr").trim_end_matches(".#ft");
        let Some(d) = drawable::parse_model(v, &r.phys, rage::joaat(stem)) else { continue };
        // Find the base the parse used: by name.
        let b = (0..le::u16(v, 0x38) as usize).find_map(|k| drawable::dictionary_entry(v, k).filter(|&b| drawable::name(v, b) == d.name)).or(drawable::base(v)).unwrap_or(0);
        let Some(sg) = vptr(le::u64(v, b + 0x10)) else { continue };
        let Some(sa) = vptr(le::u64(v, sg + 0x10)) else { continue };
        let n = le::u16(v, sg + 0x18) as usize;
        let decal_name = name.contains("decal") || name.contains("_dcl");
        for si in 0..n.min(1024) {
            let Some(sh) = vptr(le::u64(v, sa + si * 8)) else { continue };
            let effect = le::u32(v, sh);
            let mut area = 0.0f64;
            for m in d.meshes.iter().filter(|m| m.shader == si) {
                for t in m.indices.as_chunks::<3>().0 {
                    let p = [0, 1, 2].map(|k| m.positions[t[k] as usize].map(|c| c as f64));
                    let (u, w) = ([0, 1, 2].map(|c| p[1][c] - p[0][c]), [0, 1, 2].map(|c| p[2][c] - p[0][c]));
                    let x = [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]];
                    area += 0.5 * (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt();
                }
            }
            let albedo = match d.shaders.get(si).map(|c| c.as_slice()).unwrap_or(&[]) {
                [] => 0,
                c if c.iter().any(|t| match t {
                    drawable::TexRef::Key(k) => decodable(*k) == Some(true),
                    drawable::TexRef::Embedded(_) => true,
                }) =>
                {
                    2
                }
                _ => 1,
            };
            if std::env::var_os("GREY").is_some() && albedo < 2 && area > 2000.0 {
                println!(
                    "grey {:>10.0} m2  {effect:08x} bucket {:02x} {} {}  shader {si}  candidates {:?}",
                    area,
                    le::u8(v, sh + 0x39),
                    if albedo == 0 { "no-albedo" } else { "bcpack" },
                    d.name,
                    d.shaders.get(si)
                );
            }
            let e = stats.entry(effect).or_default();
            e.0 += 1;
            e.1 += area;
            e.2 += decal_name as usize;
            e.3[albedo] += area;
            if e.4.len() < 3 && !e.4.iter().any(|x| x == &d.name) {
                e.4.push(d.name.clone());
            }
        }
    }
    let mut v: Vec<_> = stats.into_iter().collect();
    v.sort_by(|a, b| b.1.1.total_cmp(&a.1.1));
    let total: f64 = v.iter().map(|x| x.1.1).sum();
    println!("effect    shaders  area%  decal-named  albedo none/bcpack/ok (area%)  name  examples");
    for (h, (n, area, dn, al, ex)) in v.iter().take(70) {
        let p = |x: f64| 100.0 * x / area.max(1e-9);
        println!("{h:08x} {n:7} {:6.2} {dn:6}  {:5.1} {:5.1} {:5.1}  {}  {}", 100.0 * area / total, p(al[0]), p(al[1]), p(al[2]), known.get(h).map_or("?", |s| s.as_str()), ex.join(" "));
    }
}

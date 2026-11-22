//! Checks against the real game files, found through `VICE_LENS_GAME`.
//! Skipped when it is not set: the files are the user's own and are not in
//! this repository.

use std::path::PathBuf;

use rage::{Archive, gfx, rbf, rsc, text, txd};

fn game() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("VICE_LENS_GAME")?);
    dir.join("data.rpf").exists().then_some(dir)
}

fn file_at(archive: &Archive, offset: u64) -> Vec<u8> {
    let mut streams = Vec::new();
    archive.scan(|s, _| {
        if s.offset >= offset {
            streams.push(s);
        }
        streams.len() < 256
    });
    archive.read_file(&Archive::files(&streams)[0]).unwrap()
}

#[test]
fn english_labels_resolve() {
    let Some(g) = game() else { return };
    let a = Archive::open(g.join("data.rpf")).unwrap();
    let t = text::parse(&file_at(&a, 0x19be010)).unwrap();
    assert_eq!(t.len(), 36888);
    assert_eq!(t.get("PM_INDX_GAME"), Some("Game"));
    assert_eq!(t.get("PM_INDX_MAP"), Some("Map"));
    assert_eq!(t.get("PM_INDX_SET"), Some("Settings"));
    assert_eq!(t.get("IB_QUIT"), Some("Quit"));
}

#[test]
fn ui_documents_parse() {
    let Some(g) = game() else { return };
    let a = Archive::open(g.join("data.rpf")).unwrap();
    let mut streams = Vec::new();
    a.scan(|s, _| {
        streams.push(s);
        true
    });
    let (mut ok, mut failed, mut pause) = (0, 0, false);
    for f in Archive::files(&streams) {
        let d = a.read_file(&f).unwrap();
        if !rbf::is_rbf(&d) {
            continue;
        }
        match rbf::parse(&d) {
            Ok(root) => {
                ok += 1;
                pause |= root.name == "UIScreen" && root.text_of("Name").as_deref() == Some("SCRN_PauseMenu");
            }
            Err(_) => failed += 1,
        }
    }
    assert!(ok >= 1259, "parsed {ok}");
    assert!(failed <= 3, "failed {failed}");
    assert!(pause, "SCRN_PauseMenu not found");
}

#[test]
fn fonts_have_glyphs() {
    let Some(g) = game() else { return };
    let a = Archive::open(g.join("data.rpf")).unwrap();
    let fonts = gfx::fonts(&file_at(&a, 0xadf3000)).unwrap();
    let futura = fonts.iter().find(|f| f.name == "futura_book").expect("futura_book");
    let g = futura.glyph('G').unwrap();
    assert!(g.advance > 0.0 && !g.path.is_empty());
    assert!(futura.ascent > 0.0);
}

#[test]
fn pause_menu_icons_decode() {
    let Some(g) = game() else { return };
    let a = Archive::open(g.join("textures.rpf")).unwrap();
    let mut streams = Vec::new();
    a.scan(|s, _| {
        streams.push(s);
        true
    });
    let first = *streams.iter().find(|s| s.offset == 0x916c010).unwrap();
    let next = |c: u64| streams.iter().find(|s| s.offset >= (c + 15) & !15).copied();
    let res = rsc::read(&a, first, next).unwrap();
    let texs = txd::textures(&res.virt);
    assert_eq!(texs.len(), 46);
    let allocs = txd::allocations(&texs, res.phys.len());
    let (i, t) = texs.iter().enumerate().find(|(_, t)| t.name == "index_map").unwrap();
    let px = txd::top_mip(&res.phys, t, allocs[i]).unwrap();
    assert_eq!(px.len(), 64 * 64); // BC3: 16 blocks × 16 blocks × 16 bytes
    assert!(px.iter().any(|&b| b != 0));
}

#[test]
fn style_store_pso_blocks_tile_psin() {
    let Some(g) = game() else { return };
    let a = Archive::open(g.join("data.rpf")).unwrap();
    let styles = rage::pso::Pso::parse(file_at(&a, 0x1c33f000)).unwrap();
    let conversions = rage::pso::Pso::parse(file_at(&a, 0x1c330000)).unwrap();
    assert_eq!(styles.blocks.len(), 30);
    assert_eq!(conversions.blocks.len(), 21);
    // The style map holds MENU_HEADER; its array resolves into block 1.
    let map = styles.block_of_type(0xebbc_845f).unwrap();
    let entry = (map.offset..map.offset + map.size).step_by(24).find(|&o| styles.u32(o) == rage::joaat("MENU_HEADER")).unwrap();
    let (_, block, _) = styles.resolve(styles.u64(entry + 8)).unwrap();
    assert_eq!(block, 1);
}

/// Mean difference between neighbouring bytes of a row-major image with
/// `bpe` bytes per pixel: low for a picture, high for a scrambled one.
fn roughness(img: &[u8], w: usize, h: usize, bpe: usize) -> f64 {
    let mut s = 0u64;
    for y in 0..h {
        for x in 0..w {
            for b in 0..bpe {
                let v = img[(y * w + x) * bpe + b] as i32;
                if x + 1 < w {
                    s += (v - img[(y * w + x + 1) * bpe + b] as i32).unsigned_abs() as u64;
                }
                if y + 1 < h {
                    s += (v - img[((y + 1) * w + x) * bpe + b] as i32).unsigned_abs() as u64;
                }
            }
        }
    }
    s as f64 / (w * h * bpe) as f64
}

/// Tiled textures of one and four bytes a pixel come out as pictures: the
/// dashboard's seven-segment digits (A8) and the gradient maps (BGRA8).
/// Scrambled, they measure about four times rougher.
#[test]
fn uncompressed_textures_untile() {
    let Some(g) = game() else { return };
    for (archive, dict, name, bpe, most) in [("data.rpf", 0x11b2d010, "generic_instruments_i8b", 1, 12.0), ("textures.rpf", 0x225b010, "gradientmaps", 4, 4.0)] {
        let a = Archive::open(g.join(archive)).unwrap();
        let r = a.resource_at(dict).unwrap();
        let texs = txd::textures(&r.virt);
        let allocs = txd::allocations(&texs, r.phys.len());
        let i = texs.iter().position(|t| t.name == name).unwrap();
        let t = &texs[i];
        let px = txd::top_mip(&r.phys, t, allocs[i]).unwrap();
        let rough = roughness(&px, t.width as usize, t.height as usize, bpe);
        assert!(rough < most, "{name}: roughness {rough:.2}, want under {most}");
    }
}

/// A resource block over 32 MiB is stored as several streams, of which only
/// the last is not exactly 32 MiB: the textures after such a block (here at
/// the end of two large dictionaries) are read too.
#[test]
fn blocks_over_32_mib_read_whole() {
    let Some(g) = game() else { return };
    let a = Archive::open(g.join("textures.rpf")).unwrap();
    for (dict, name) in [(0x2263010, "deadeye_cross"), (0xaa3010, "overlay_screenpixels_2x2_bw")] {
        let r = a.resource_at(dict).unwrap();
        let texs = txd::textures(&r.virt);
        let allocs = txd::allocations(&texs, r.phys.len());
        let i = texs.iter().position(|t| t.name == name).unwrap();
        let t = &texs[i];
        assert!(t.data.is_some_and(|d| d < r.phys.len()), "{name}: data at {:?}, physical segment {} bytes", t.data, r.phys.len());
        assert!(txd::top_mip(&r.phys, t, allocs[i]).is_some(), "{name}: no pixels");
    }
}

/// Each shader's render bucket (the byte at +0x39) tells decals, glass,
/// water and solid surfaces apart.
#[test]
fn render_buckets_tell_materials_apart() {
    use rage::drawable::{self, Bucket};
    let Some(g) = game() else { return };
    for (archive, offset, want) in [
        ("pack/core_restofworld/core_restofworld.rpf", 0x217d010u64, Bucket::Decal),
        ("levels.rpf", 0x1ad8ad010, Bucket::Alpha),
        ("levels.rpf", 0x13f745010, Bucket::Water),
        ("levels.rpf", 0x2fc338010, Bucket::Opaque),
    ] {
        let a = Archive::open(g.join(archive)).unwrap();
        let r = a.resource_at(offset).unwrap();
        let d = drawable::parse(&r.virt, &r.phys).unwrap();
        assert_eq!(d.buckets.first(), Some(&want), "{}: buckets {:?}", d.name, d.buckets);
    }
}

/// A shader names every texture it uses: a bin's colour map, its dirt (a
/// second colour map), normals, specular, and a tint palette, the map that
/// colours the bins differently. The palettes are in none of the texture
/// packs (of 1 701 seen, one is).
#[test]
fn shaders_name_their_textures_and_tint_palettes_lie_elsewhere() {
    use rage::drawable::{self, TexRef};
    let Some(g) = game() else { return };
    let a = Archive::open(g.join("levels.rpf")).unwrap();
    let r = a.resource_at(0x10_4fd0_6010).unwrap();
    let d = drawable::parse_model(&r.virt, &r.phys, rage::joaat("prop_am_bin_04a")).unwrap();
    let p = &d.params[0];
    let key = |name: &str| p.iter().find(|(h, _)| *h == rage::joaat(name)).map(|t| t.1.clone());
    assert_eq!(key("diffusetex"), Some(TexRef::Key(0x2908_9e7b_b654_4d24)));
    assert!(key("diffusetex2").is_some() && key("bumptex").is_some() && key("speculartex").is_some());
    let Some(TexRef::Key(palette)) = key("tintpalettetex") else { panic!("no tint palette: {p:?}") };
    let packs = ["props", "maps", "interiors", "terrain"].map(|d| g.join("data/ttdb").join(d));
    for dir in packs {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let pack = rage::ttpf::Pack::open(e.path()).unwrap();
            assert!(pack.find(palette).is_none(), "palette {palette:016x} in {}", e.path().display());
        }
    }
    // The colour map is the first albedo candidate.
    assert_eq!(d.shaders[0].first(), Some(&TexRef::Key(0x2908_9e7b_b654_4d24)));
}

/// A vehicle's skeleton names its parts and places them: the wheels sit
/// at their bones.
#[test]
fn vehicle_skeletons_place_the_wheels() {
    use rage::drawable;
    let Some(g) = game() else { return };
    let a = Archive::open(g.join("levels.rpf")).unwrap();
    let r = a.resource_at(0x11_3fc5_9010).unwrap();
    let d = drawable::parse_model(&r.virt, &r.phys, rage::joaat("infernus2_hi")).unwrap();
    assert_eq!(d.bones.len(), 333);
    assert_eq!(d.bones[0].name, "chassis");
    let at = |n: &str| d.bones.iter().find(|b| b.name == n).map(|b| [b.matrix[12], b.matrix[13], b.matrix[14]]).unwrap();
    // Left front: −x, +y, below the middle; the driver's seat on the left,
    // the steering wheel above it.
    let lf = at("wheel_lf");
    assert!((lf[0] + 0.777).abs() < 0.01 && (lf[1] - 1.336).abs() < 0.01 && (lf[2] + 0.223).abs() < 0.01, "{lf:?}");
    let (seat, wheel) = (at("seat_dside_f"), at("steeringwheel"));
    assert!(seat[0] < 0.0 && wheel[0] < 0.0 && wheel[2] > seat[2], "{seat:?} {wheel:?}");
    // Left and right mirror each other.
    let rf = at("wheel_rf");
    assert!((rf[0] + lf[0]).abs() < 0.01 && (rf[1] - lf[1]).abs() < 0.01, "{rf:?}");
    for n in ["bodyshell", "bonnet", "boot", "door_dside_f", "steeringwheel"] {
        assert!(d.bones.iter().any(|b| b.name == n), "{n}");
    }
    // A pickup's skeleton reads the same way.
    let r = a.resource_at(0x11_0598_e010).unwrap();
    let d = drawable::parse_model(&r.virt, &r.phys, rage::joaat("bison_hi")).unwrap();
    assert_eq!(d.bones.len(), 279);
    assert!(d.bones.iter().all(|b| !b.name.is_empty()));
    assert!(d.bones.iter().any(|b| b.name == "wheel_lr"));
}

/// Every vertex of an animal follows up to eight bones of the skeleton in
/// the animal's own model, through the body's palette of bone name hashes.
#[test]
fn animals_are_skinned_through_a_palette_of_bone_names() {
    use rage::drawable;
    let Some(g) = game() else { return };
    let a = Archive::open(g.join("packs.rpf")).unwrap();
    let r = a.resource_at(0x5603_3010).unwrap();
    let body = drawable::parse_model(&r.virt, &r.phys, rage::joaat("crm_dog_chihuahua_f_00_body_00")).unwrap();
    let r = a.resource_at(0x4595_f010).unwrap();
    let skel = drawable::parse_model(&r.virt, &r.phys, rage::joaat("crm_dog_chihuahua_f_00")).unwrap().bones;
    assert_eq!(skel.len(), 142);
    let m = &body.meshes[0];
    assert_eq!(m.palette.len(), 71);
    assert!(m.palette.windows(2).all(|w| w[0] < w[1]));
    assert_eq!((m.joints.len(), m.weights.len()), (m.positions.len(), m.positions.len()));
    let by_hash: std::collections::HashMap<u32, usize> = skel.iter().enumerate().map(|(i, b)| (rage::joaat(&b.name), i)).collect();
    assert!(m.palette.iter().filter(|h| by_hash.contains_key(h)).count() >= 70);
    // Each vertex's weights sum to 255 and its heaviest bone lies near it.
    let mut far = 0.0f64;
    for (v, (j, w)) in m.joints.iter().zip(&m.weights).enumerate() {
        assert_eq!(w.iter().map(|&x| x as u32).sum::<u32>(), 255);
        let k = (0..8).max_by_key(|&k| w[k]).unwrap();
        let b = &skel[by_hash[&m.palette[j[k] as usize]]].matrix;
        let p = m.positions[v];
        far += (((p[0] - b[12]).powi(2) + (p[1] - b[13]).powi(2) + (p[2] - b[14]).powi(2)) as f64).sqrt();
    }
    assert!(far / (m.positions.len() as f64) < 0.1, "mean {}", far / m.positions.len() as f64);
}

/// A vehicle's pieces each follow one of its own bones (no palette): the
/// driver's door behind its hinge, outside the middle.
#[test]
fn vehicle_pieces_follow_their_own_bones() {
    use rage::drawable;
    let Some(g) = game() else { return };
    let a = Archive::open(g.join("levels.rpf")).unwrap();
    let r = a.resource_at(0x11_3fc5_9010).unwrap();
    let d = drawable::parse_model(&r.virt, &r.phys, rage::joaat("infernus2_hi")).unwrap();
    let door = d.bones.iter().position(|b| b.name == "door_dside_f").unwrap() as u16;
    let (mut n, mut sum) = (0usize, [0.0f32; 3]);
    for m in d.meshes.iter().filter(|m| !m.joints.is_empty()) {
        assert!(m.palette.is_empty());
        for (v, j) in m.joints.iter().enumerate() {
            if j[0] == door {
                n += 1;
                (0..3).for_each(|c| sum[c] += m.positions[v][c]);
            }
        }
    }
    let mid = sum.map(|x| x / n as f32);
    let hinge = &d.bones[door as usize].matrix;
    assert!(n > 1000, "{n}");
    assert!(mid[0] < hinge[12] && mid[1] < hinge[13], "{mid:?}");
}

/// Clothes carry cloth bones in skeletons after their first; with the
/// body's skeleton they cover the whole palette.
#[test]
fn clothes_carry_cloth_bones() {
    use rage::drawable;
    let Some(g) = game() else { return };
    let a = Archive::open(g.join("packs.rpf")).unwrap();
    let r = a.resource_at(0xe6d6_4010).unwrap();
    let shirt = drawable::parse_model(&r.virt, &r.phys, rage::joaat("tshirtcrewneck_mr1_000_slim_cc_tu_long_full")).unwrap();
    let r = a.resource_at(0x454d_b010).unwrap();
    let body = drawable::parse_model(&r.virt, &r.phys, rage::joaat("ambient_malebody")).unwrap().bones;
    assert_eq!(body.len(), 560);
    assert_eq!(shirt.bones.len() + shirt.more_bones.len(), 160);
    let known: std::collections::HashSet<u32> = body.iter().chain(&shirt.bones).chain(&shirt.more_bones).map(|b| rage::joaat(&b.name)).collect();
    let pal = &shirt.meshes[0].palette;
    assert_eq!(pal.len(), 115);
    assert!(pal.iter().all(|h| known.contains(h)));
}

/// A clip of the earlier channel types decodes: a chicken stepping up to
/// eat moves its bones with unit rotations, starting from the skeleton's
/// own pose; a locomotion clip whose motion is in type 10 says it is not
/// whole.
#[test]
fn clips_play() {
    use rage::{anim, drawable};
    let Some(g) = game() else { return };
    let a = Archive::open(g.join("anim.rpf")).unwrap();
    let v = a.resource_at(0x3761_9010).unwrap().virt;
    let (name, clips) = anim::dictionary(&v).unwrap();
    assert_eq!(name, "ambient@scenarios@creatures@air@bird@chicken@land@eat@stand_enter");
    assert_eq!(clips.len(), 1);
    let c = &clips[0];
    assert_eq!(c.name, "pack:/enter.clip");
    assert!((c.length - 0.833).abs() < 0.01 && anim::readable(&v, c));
    let (n, poses) = anim::sample(&v, c, 30.0);
    assert_eq!(n, 26);
    let rotated: Vec<_> = poses.iter().filter(|(_, p)| p.iter().all(|x| x.rotation.is_some())).collect();
    assert!(rotated.len() >= 80, "{}", rotated.len());
    for (_, p) in &rotated {
        for x in p.iter() {
            let q = x.rotation.unwrap();
            assert!((q.iter().map(|c| c * c).sum::<f32>() - 1.0).abs() < 1e-3);
        }
    }
    // The pelvis starts where the chicken's skeleton holds it: the same
    // turn relative to its parent (a quaternion and its negative agree).
    let p = a_skeleton_turn(&g, "SKEL_Pelvis");
    let q = poses[&rage::joaat("SKEL_Pelvis")][0].rotation.unwrap();
    let dot: f32 = p.iter().zip(&q).map(|(a, b)| a * b).sum();
    assert!(dot.abs() > 0.99, "{p:?} {q:?}");
    // Most locomotion keeps its motion in type 10, which decodes too.
    let v = a.resource_at(0x1_0023_a010).unwrap().virt;
    let (_, clips) = anim::dictionary(&v).unwrap();
    assert_eq!(clips.len(), 30);
    assert!(clips.iter().all(|c| anim::readable(&v, c)));

    fn a_skeleton_turn(g: &std::path::Path, bone: &str) -> [f32; 4] {
        let a = Archive::open(g.join("packs.rpf")).unwrap();
        let r = a.resource_at(0x455a_e010).unwrap();
        let bones = drawable::parse_model(&r.virt, &r.phys, rage::joaat("crb_chicken_cubalaya_f_00")).unwrap().bones;
        let i = bones.iter().position(|b| b.name == bone).unwrap();
        let (m, p) = (bones[i].matrix, bones[bones[i].parent.unwrap() as usize].matrix);
        // Local rotation: the parent's rotation transposed times the bone's
        // (rows of each matrix are its axes).
        let r = |m: &[f32; 16], row: usize, col: usize| m[row * 4 + col];
        let l: [[f32; 3]; 3] = std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| r(&m, i, k) * r(&p, j, k)).sum()));
        // l[i][j]: bone axis i in the parent's frame, component j; as a
        // matrix with columns the axes, M[j][i] = l[i][j].
        let mm = |row: usize, col: usize| l[col][row];
        let w = (1.0 + mm(0, 0) + mm(1, 1) + mm(2, 2)).max(0.0).sqrt() / 2.0;
        [(mm(2, 1) - mm(1, 2)) / (4.0 * w), (mm(0, 2) - mm(2, 0)) / (4.0 * w), (mm(1, 0) - mm(0, 1)) / (4.0 * w), w]
    }
}

/// Type 10 (incremental curves): a clip that keeps part of its motion in
/// them and its LOD copy, which keeps all of it in the earlier types, agree
/// at the copy's keys (every half second).
#[test]
fn incremental_curves_agree_with_their_lod_copy() {
    use rage::anim;
    let Some(g) = game() else { return };
    let a = Archive::open(g.join("anim.rpf")).unwrap();
    let pose = |o: u64, name: &str| {
        let v = a.resource_at(o).unwrap().virt;
        let (n, clips) = anim::dictionary(&v).unwrap();
        assert_eq!(n, name);
        anim::sample(&v, &clips[0], 2.0)
    };
    let (n, full) = pose(0x31f7_1010, "ambient@scenarios@beachchair@lying@cellphone@male_a@average@base");
    let (m, lod) = pose(0x31f7_5010, "ambient@scenarios@beachchair@lying@cellphone@male_a@average@base@lod");
    assert_eq!((n, m), (9, 9));
    let mut turns = Vec::new();
    for (bone, p) in &full {
        let Some(q) = lod.get(bone) else { continue };
        for (x, y) in p.iter().zip(q) {
            if let (Some(a), Some(b)) = (x.rotation, y.rotation) {
                let dot: f32 = a.iter().zip(&b).map(|(a, b)| a * b).sum();
                turns.push(2.0 * dot.abs().min(1.0).acos().to_degrees());
            }
        }
    }
    let mean = turns.iter().sum::<f32>() / turns.len() as f32;
    let worst = turns.iter().copied().fold(0.0, f32::max);
    assert!(turns.len() > 500 && mean < 0.5 && worst < 3.0, "{} turns, mean {mean}°, worst {worst}°", turns.len());
}

/// A dictionary without clips holds motions: `jog_run_stop` plays the 26
/// clips of Lucia's run-stop dictionary, named by hashes of their names,
/// each with how far is left to go and the feet's phase over its time.
#[test]
fn motions_name_clips_of_other_dictionaries() {
    use rage::anim;
    let Some(g) = game() else { return };
    let a = Archive::open(g.join("anim.rpf")).unwrap();
    let v = a.resource_at(0x1_02f5_e010).unwrap().virt;
    let (name, clips) = anim::dictionary(&v).unwrap();
    assert_eq!(name, "gameplay@locomotion@female_character@lucia@jog@run");
    assert!(clips.is_empty());
    let motions = anim::motions(&v);
    assert_eq!(motions.len(), 1);
    let m = &motions[0];
    assert_eq!(m.name, "pack:/jog_run_stop.motion");
    assert_eq!(m.sources.len(), 26);
    let dict = rage::joaat("gameplay@locomotion@female_character@lucia@jog@standing@run_stop");
    assert!(m.sources.iter().all(|s| s.dictionary == dict));
    let w = a.resource_at(0x987b_2010).unwrap().virt;
    let (_, stops) = anim::dictionary(&w).unwrap();
    let keys: std::collections::HashSet<u32> = stops.iter().map(|c| anim::clip_key(&c.name)).collect();
    assert!(m.sources.iter().all(|s| keys.contains(&s.clip)));
    // Distance falls to 0; the phase rises by π a step.
    let s = &m.sources[0];
    assert_eq!(s.params.len(), 2);
    let (d, p) = (&s.params[0], &s.params[1]);
    assert!((d[0].0 - 1.977).abs() < 0.01 && d.last().unwrap().0.abs() < 1e-3);
    assert!(p.last().unwrap().0 > 30.0 && p.windows(2).all(|k| k[1].1 >= k[0].1));
}

/// A cutscene's shot (`cin@…`, in the story pack) holds animations
/// without clips, each for one actor and played whole.
#[test]
fn cutscene_shots_hold_whole_animations() {
    use rage::anim;
    let Some(g) = game() else { return };
    let a = Archive::open(g.join("pack/story000_restofworld/story000_restofworld.rpf")).unwrap();
    let v = a.resource_at(0x25_7310).unwrap().virt;
    let (name, clips) = anim::dictionary(&v).unwrap();
    assert_eq!(name, "cin@ban_carte_ext_p1_t01-1");
    assert_eq!(clips.len(), 1);
    assert_eq!(clips[0].name, "pack:/cinematic_q8@crr_alligator_american_m_00@0@60_0.anim");
    assert!((clips[0].length - 1.033).abs() < 0.01 && anim::readable(&v, &clips[0]));
    let (n, poses) = anim::sample(&v, &clips[0], 30.0);
    assert_eq!(n, 32);
    assert!(poses.len() > 20, "{}", poses.len());
}

/// Many textures keep only their small mips (stored first) in the console's
/// BCPack codec: the top level is zlib and decodes.
#[test]
fn top_level_decodes_past_bcpack_mips() {
    let Some(g) = game() else { return };
    let p = rage::ttpf::Pack::open(g.join("data/ttdb/terrain/ttdb_terrain_4_7.ttpf")).unwrap();
    let i = p.find(0x62a6_c639_3f13_a67c).unwrap();
    assert!(!p.plain(i), "the small mips are BCPack");
    assert!(p.top_plain(i));
    assert_eq!(p.top_mip(i).unwrap().len(), 512 * 512 / 2);
}

/// The terrain blend shader's meshes carry a second UV set that tiles the
/// layers; the first spans the piece once.
#[test]
fn terrain_layers_tile_by_the_second_uv_set() {
    let Some(g) = game() else { return };
    let a = Archive::open(g.join("levels.rpf")).unwrap();
    let r = a.resource_at(0xd_4584_7010).unwrap();
    let d = rage::drawable::parse(&r.virt, &r.phys).unwrap();
    let m = d.meshes.iter().find(|m| d.layered[m.shader]).expect("a terrain blend mesh");
    let span = |uv: &[[f32; 2]]| uv.iter().map(|u| u[1]).fold(f32::MIN, f32::max) - uv.iter().map(|u| u[1]).fold(f32::MAX, f32::min);
    assert_eq!(m.uvs1.len(), m.positions.len());
    assert!(span(&m.uvs) <= 1.01 && span(&m.uvs1) > 4.0, "UV0 spans {}, UV1 {}", span(&m.uvs), span(&m.uvs1));
}

/// A small mipmapped texture's levels each take whole 4 KiB tiles, so its
/// allocation is well over 4/3 of its top level (here 28 KiB for a 16 KiB
/// top): the top level still ends the allocation.
#[test]
fn small_textures_keep_their_top_level_at_the_end() {
    let Some(g) = game() else { return };
    let p = rage::ttpf::Pack::open(g.join("data/ttdb/vfx/ttdb_vfx.ttpf")).unwrap();
    let i = p.find(0x2085_11db_3f7a_dc71).unwrap();
    let e = &p.entries[i];
    assert_eq!((e.width, e.height, e.mips, e.alloc), (128, 256, true, 0x7000));
    assert!(!p.top_at_start(i));
}

/// Interiors: a resource defining two variants of a building lists each
/// one's entities (83 and 99, all 182 of the resource); a placement names its
/// interior by the 64-bit hash of the definition.
#[test]
fn interiors_and_their_placements_read() {
    let Some(g) = game() else { return };
    let a = Archive::open(g.join("levels.rpf")).unwrap();
    let r = a.resource_at(0x1_2088_3010).unwrap();
    let mlos = rage::mapdata::mlos(&r.virt, &r.phys);
    assert_eq!(mlos.iter().map(|m| m.entities.len()).collect::<Vec<_>>(), [83, 99]);
    let r = a.resource_at(0x2_3e36_b010).unwrap();
    let placed = rage::mapdata::mlo_instances(&r.virt, &r.phys);
    let shop = placed.iter().find(|p| p.hash == 0x7ba4_1238_1f59_1fe3).expect("the shop's placement");
    assert!((shop.place.pos[0] + 648.406).abs() < 0.01 && (shop.place.pos[1] - 215.623).abs() < 0.01);
}

/// The largest texture packs pass 4 GiB: a texture's offset has 21 bits of
/// 4 KiB pages. Read with 20, this roof plate came from 4 GiB lower, lime
/// noise instead of grey steel.
#[test]
fn textures_past_4_gib_read_from_their_place() {
    let Some(g) = game() else { return };
    let p = rage::ttpf::Pack::open(g.join("data/ttdb/maps/ttdb_maps_2_7.ttpf")).unwrap();
    let i = p.find(0xf2e4_ab1e_fc8b_de92).unwrap();
    assert!(p.entries[i].offset >= 1 << 32);
    assert!(p.top_plain(i));
    assert_eq!(p.top_mip(i).unwrap().len(), 256 * 256);
}

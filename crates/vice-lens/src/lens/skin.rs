//! Vice Lens: skeletons and skinning, for posing and moving models in the
//! viewer. A vehicle's pieces follow its own bones (each vertex names one);
//! characters and animals are skinned against a skeleton in another model
//! (`ambient_malebody`, `crm_dog_chihuahua_f_00`), matched through each
//! model's palette of bone name hashes. A bone the skeleton lacks rides
//! its nearest ancestor the skeleton has (a jaw's facial rig the head), else
//! the body part its name gives (a shirt's cloth bone
//! `GM_U01_L_UpperArm01_O` rides `SKEL_L_UpperArm`), else the `SKEL_` bone
//! nearest the vertex.

use std::collections::HashMap;

use rage::drawable::{Bone, Drawable, Mesh};

use super::Lens;
use super::http::{Reply, Request, quote};
use crate::world;

/// Per vertex: four bones of the skeleton and their weights, summing to 255.
pub(super) type Skin4 = (Vec<[u16; 4]>, Vec<[u8; 4]>);

/// Per vertex: every bone of the skeleton it follows and its summed weight.
type Blend = Vec<Vec<(u16, u32)>>;

/// The bone nearest a point among some (the main bones for None).
type Nearest<'a> = &'a dyn Fn([f32; 3], Option<&[u16]>) -> u16;

fn pos(b: &Bone) -> [f32; 3] {
    [b.matrix[12], b.matrix[13], b.matrix[14]]
}

/// Where a model's bone goes on a skeleton: a bone of the skeleton, the
/// one nearest the vertex among some (the body part a cloth bone names), or
/// the main bone nearest the vertex.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Ride {
    Bone(u16),
    Among(Vec<u16>),
    Nearest,
}

/// Each mesh's vertices on the bones of `skel` (None for a mesh that
/// follows no bone). `own`: `skel` is the model's own skeleton, which a
/// mesh without a palette indexes directly. `kin`: bones that name a
/// part's palette when the part has no skeleton of its own (a dog's teeth
/// name the facial rig of its head). A vertex's bone the skeleton lacks
/// rides its nearest ancestor in the model's own skeleton that the skeleton
/// has, else the body part its name (or its chain's top's) gives (see
/// `rides`), keeping the blend of the vertex's bones; else the main
/// (`SKEL_`) bone nearest the vertex, measured to the bone's length (a
/// point low on the thigh lies nearer the knee's place than the hip's, but
/// on the thigh). On another model's skeleton, vertices at one place (the
/// two sides of a seam) take their bones together, so seams stay closed; a
/// vehicle's pieces, on their own bones, keep theirs.
pub(super) fn skin(d: &Drawable, skel: &[Bone], own: bool, kin: &[Bone]) -> Vec<Option<Skin4>> {
    let by_hash: HashMap<u32, usize> = skel.iter().enumerate().map(|(i, b)| (rage::joaat(&b.name), i)).collect();
    // The model's own bones by hash (cloth bones in the further skeletons),
    // and those of a model beside it for a part that has none (`kin`).
    let names: HashMap<u32, &str> = kin.iter().chain(&d.bones).chain(&d.more_bones).map(|b| (rage::joaat(&b.name), b.name.as_str())).collect();
    let lengths = lengths(skel);
    let nearest = |p: [f32; 3], among: Option<&[u16]>| -> u16 {
        match among {
            Some(v) => v.iter().copied().min_by(|&a, &b| dist(pos(&skel[a as usize]), p).total_cmp(&dist(pos(&skel[b as usize]), p))).unwrap_or(0),
            None => lengths.iter().min_by(|a, b| to_segment(p, a.1, a.2).total_cmp(&to_segment(p, b.1, b.2))).map_or(0, |x| x.0),
        }
    };
    // A bone the skeleton lacks: up its own skeleton to the first bone the
    // skeleton has (a jaw's facial rig to the head), else by the name of
    // the bone, or of the top of its chain.
    let first: HashMap<u32, usize> = d.bones.iter().enumerate().map(|(i, b)| (rage::joaat(&b.name), i)).collect();
    let ride = |h: u32| -> Ride {
        let mut tried = names.get(&h).copied().into_iter().collect::<Vec<_>>();
        if let Some(&i) = first.get(&h) {
            let mut at = i;
            for _ in 0..d.bones.len() {
                let Some(p) = d.bones[at].parent.map(usize::from).filter(|&p| p < d.bones.len()) else { break };
                at = p;
                if let Some(&k) = by_hash.get(&rage::joaat(&d.bones[at].name)) {
                    return Ride::Bone(k as u16);
                }
            }
            tried.push(&d.bones[at].name);
        }
        tried.iter().map(|n| rides(n, skel)).find(|v| !v.is_empty()).map_or(Ride::Nearest, Ride::Among)
    };
    let mut maps: HashMap<Vec<u32>, Vec<Ride>> = HashMap::new();
    let mut acc: Vec<Option<Blend>> = d
        .meshes
        .iter()
        .map(|m| {
            if m.joints.is_empty() || skel.is_empty() {
                return None;
            }
            // Local bone → where it goes on the skeleton.
            let map = maps
                .entry(m.palette.clone())
                .or_insert_with(|| {
                    let local: Vec<u32> = if m.palette.is_empty() { d.bones.iter().map(|b| rage::joaat(&b.name)).collect() } else { m.palette.clone() };
                    local
                        .iter()
                        .enumerate()
                        .map(|(i, h)| match by_hash.get(h) {
                            Some(&k) => Ride::Bone(k as u16),
                            None if own && m.palette.is_empty() && i < skel.len() => Ride::Bone(i as u16),
                            None => ride(*h),
                        })
                        .collect()
                })
                .clone();
            Some(blend(m, &map, &nearest))
        })
        .collect();
    if !own {
        weld(d, &mut acc);
    }
    acc.into_iter().map(|a| a.map(|a| a.iter().map(|x| top4(x)).unzip())).collect()
}

/// Vertices at one place (to a twentieth of a millimetre), in any mesh,
/// take the sum of their bones: the two sides of a seam stay together.
fn weld(d: &Drawable, acc: &mut [Option<Blend>]) {
    let mut at: HashMap<[i32; 3], Vec<(usize, usize)>> = HashMap::new();
    for (mi, a) in acc.iter().enumerate() {
        if a.is_some() {
            for (v, p) in d.meshes[mi].positions.iter().enumerate() {
                at.entry(p.map(|x| (x * 20000.0).round() as i32)).or_default().push((mi, v));
            }
        }
    }
    for group in at.values().filter(|g| g.len() > 1) {
        let mut sum: Vec<(u16, u32)> = Vec::new();
        for &(mi, v) in group {
            for &(b, w) in acc[mi].as_ref().and_then(|a| a.get(v)).into_iter().flatten() {
                match sum.iter_mut().find(|x| x.0 == b) {
                    Some(x) => x.1 += w,
                    None => sum.push((b, w)),
                }
            }
        }
        for &(mi, v) in group {
            if let Some(x) = acc[mi].as_mut().and_then(|a| a.get_mut(v)) {
                x.clone_from(&sum);
            }
        }
    }
}

/// A bone name's side and body part: `SKEL_L_Calf` → (L, `Calf`),
/// `SKEL_Spine5` → (C, `Spine5`), `BM_R_Deltoid` → (R, `Deltoid`).
fn side_part(name: &str) -> (char, &str) {
    let rest = name.split_once('_').map_or("", |x| x.1);
    match rest.split_once('_') {
        Some(("L", p)) => ('L', p),
        Some(("R", p)) => ('R', p),
        _ => ('C', rest),
    }
}

/// The skeleton bones a bone it lacks rides, by what its name says. Cloth
/// and other helper bones name the side and the body part they hang on
/// (`GM_L01_L_Calf02_CB`, `GM_U01_C_Spine5_RB`, `MH_Necklace_R_Delt_F`,
/// `FlipFlop_L_Foot`): the skeleton's main bone of that part on that side
/// (`SKEL_L_Calf`, `SKEL_Spine5`; `SKEL_L_Thigh` for a `ThighButt`), else
/// its other bones of that part (`MH_L_Knee`, `BM_R_Deltoid…`). A middle
/// part with no middle bone (a skirt's `C_Thigh`) rides either side's; a
/// face's rig (`FACIAL_…`, `FB_…`) the head. Empty when the name names no
/// part.
pub(super) fn rides(name: &str, skel: &[Bone]) -> Vec<u16> {
    // A face's rig (`FACIAL_C_FacialRoot`, `FB_L_Brow…`) rides the head.
    if name.starts_with("FACIAL_") || name.starts_with("FB_") {
        return skel.iter().position(|b| b.name == "SKEL_Head").map(|i| vec![i as u16]).unwrap_or_default();
    }
    let t: Vec<&str> = name.split('_').collect();
    let Some(at) = (1..t.len()).find(|&i| matches!(t[i], "L" | "R" | "C")) else { return Vec::new() };
    let Some(&part) = t.get(at + 1) else { return Vec::new() };
    let stem = |p: &str| p.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    let key = &part[..stem(part)];
    if key.len() < 3 {
        return Vec::new();
    }
    let alike = |p: &str| {
        let k = &p[..stem(p)];
        k.starts_with(key) || (k.len() >= 3 && key.starts_with(k))
    };
    let find = |sides: &[char]| -> Vec<u16> {
        let on = |main: bool, exact: bool| -> Vec<u16> {
            let of = skel.iter().enumerate().filter(|(_, b)| {
                let kind = b.name.split('_').next().unwrap_or("");
                let (s, p) = side_part(&b.name);
                sides.contains(&s) && (if main { kind == "SKEL" } else { matches!(kind, "BM" | "MH" | "RB") }) && (if exact { p == part } else { alike(p) })
            });
            of.map(|(i, _)| i as u16).collect()
        };
        [on(true, true), on(true, false), on(false, true), on(false, false)].into_iter().find(|v| !v.is_empty()).unwrap_or_default()
    };
    match t[at] {
        "L" => find(&['L']),
        "R" => find(&['R']),
        _ => Some(find(&['C'])).filter(|v| !v.is_empty()).unwrap_or_else(|| find(&['L', 'R'])),
    }
}

/// The main (`SKEL_`; every bone when none is) bones as segments: from a
/// bone's place to each main bone it leads to (its nearest main ancestor),
/// a point for a bone that leads to none.
fn lengths(skel: &[Bone]) -> Vec<(u16, [f32; 3], [f32; 3])> {
    let mut main: Vec<bool> = skel.iter().map(|b| b.name.starts_with("SKEL_")).collect();
    if !main.contains(&true) {
        main = vec![true; skel.len()];
    }
    let lead = |mut i: usize| -> Option<usize> {
        for _ in 0..skel.len() {
            i = skel[i].parent? as usize;
            if i >= skel.len() {
                return None;
            }
            if main[i] {
                return Some(i);
            }
        }
        None
    };
    let mut out = Vec::new();
    for i in (0..skel.len()).filter(|&i| main[i]) {
        let a = pos(&skel[i]);
        let ends: Vec<[f32; 3]> = (0..skel.len()).filter(|&k| main[k] && lead(k) == Some(i)).map(|k| pos(&skel[k])).collect();
        if ends.is_empty() {
            out.push((i as u16, a, a));
        }
        out.extend(ends.into_iter().map(|b| (i as u16, a, b)));
    }
    out
}

/// Squared distance from `p` to the segment `a`–`b`; past the end a hair
/// farther, so at a joint the bone that starts there wins.
fn to_segment(p: [f32; 3], a: [f32; 3], b: [f32; 3]) -> f32 {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ap = [p[0] - a[0], p[1] - a[1], p[2] - a[2]];
    let l = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
    let t = if l > 0.0 { (ap[0] * ab[0] + ap[1] * ab[1] + ap[2] * ab[2]) / l } else { 0.0 };
    let past = if t > 1.0 { 1e-6 } else { 0.0 };
    let t = t.clamp(0.0, 1.0);
    dist(p, [a[0] + ab[0] * t, a[1] + ab[1] * t, a[2] + ab[2] * t]) + past
}

/// Two column-major matrices multiplied: `a` then, in its frame, `b`.
fn mul(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    std::array::from_fn(|i| {
        let (c, r) = (i / 4, i % 4);
        (0..4).map(|k| a[k * 4 + r] * b[c * 4 + k]).sum()
    })
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

/// Per vertex its bones on the skeleton and their weights (a bone met
/// twice counted once). `nearest`: the bone nearest a point among some
/// (the main bones for None).
fn blend(m: &Mesh, map: &[Ride], nearest: Nearest) -> Blend {
    m.joints
        .iter()
        .zip(&m.weights)
        .enumerate()
        .map(|(v, (j, w))| {
            let at = m.positions.get(v).copied().unwrap_or_default();
            let mut acc: Vec<(u16, u32)> = Vec::with_capacity(8);
            for k in (0..8).filter(|&k| w[k] > 0) {
                let b = match map.get(j[k] as usize).unwrap_or(&Ride::Nearest) {
                    Ride::Bone(b) => *b,
                    Ride::Among(v) => nearest(at, Some(v)),
                    Ride::Nearest => nearest(at, None),
                };
                match acc.iter_mut().find(|x| x.0 == b) {
                    Some(x) => x.1 += w[k] as u32,
                    None => acc.push((b, w[k] as u32)),
                }
            }
            acc
        })
        .collect()
}

/// The four heaviest bones, weights scaled to sum to 255.
fn top4(acc: &[(u16, u32)]) -> ([u16; 4], [u8; 4]) {
    let mut acc = acc.to_vec();
    acc.sort_by_key(|x| (std::cmp::Reverse(x.1), x.0));
    acc.truncate(4);
    let total: u32 = acc.iter().map(|x| x.1).sum::<u32>().max(1);
    let (mut jj, mut ww) = ([0u16; 4], [0u8; 4]);
    let mut given = 0u32;
    for (k, (b, x)) in acc.iter().enumerate() {
        jj[k] = *b;
        let share = if k + 1 == acc.len() { 255 - given } else { x * 255 / total };
        ww[k] = share as u8;
        given += share;
    }
    if acc.is_empty() {
        ww[0] = 255;
    }
    (jj, ww)
}

/// The four heaviest of each vertex's eight bones, summing to 255.
#[cfg(test)]
fn four(m: &Mesh, map: &[Ride], nearest: Nearest) -> Skin4 {
    blend(m, map, nearest).iter().map(|x| top4(x)).unzip()
}

/// A mesh as sent: shader, part, positions, UVs, indices, and its bones.
pub(super) struct Out {
    pub shader: u32,
    pub part: u32,
    pub pos: Vec<[f32; 3]>,
    pub uv: Vec<[f32; 2]>,
    pub idx: Vec<u32>,
    pub skin: Option<Skin4>,
}

/// The model's meshes, split by part when `parts` gives each triangle's
/// part (one mesh per shader and part), each carrying its bones.
pub(super) fn split(d: &Drawable, parts: Option<&[Vec<u16>]>, skins: &[Option<Skin4>]) -> Vec<Out> {
    let mut out = Vec::new();
    for (mi, me) in d.meshes.iter().enumerate() {
        let layered = d.layered.get(me.shader) == Some(&true) && me.uvs1.len() == me.positions.len();
        let skin = skins.get(mi).and_then(Option::as_ref);
        let uv_of = |v: usize| if layered { me.uvs1[v] } else { me.uvs.get(v).copied().unwrap_or([0.0, 0.0]) };
        let tri_part = parts.and_then(|p| p.get(mi));
        // part → (old vertex → new), the mesh being built.
        let mut by_part: std::collections::BTreeMap<u16, (HashMap<u32, u32>, Out)> = Default::default();
        for (t, tri) in me.indices.as_chunks::<3>().0.iter().enumerate() {
            let p = tri_part.and_then(|x| x.get(t)).copied().unwrap_or(0);
            let e = by_part
                .entry(p)
                .or_insert_with(|| (HashMap::new(), Out { shader: me.shader as u32, part: p as u32, pos: Vec::new(), uv: Vec::new(), idx: Vec::new(), skin: skin.map(|_| (Vec::new(), Vec::new())) }));
            for &v in tri {
                let n = e.1.pos.len() as u32;
                let k = *e.0.entry(v).or_insert_with(|| {
                    e.1.pos.push(me.positions[v as usize]);
                    e.1.uv.push(uv_of(v as usize));
                    if let (Some(s), Some(o)) = (skin, e.1.skin.as_mut()) {
                        o.0.push(s.0.get(v as usize).copied().unwrap_or_default());
                        o.1.push(s.1.get(v as usize).copied().unwrap_or([255, 0, 0, 0]));
                    }
                    n
                });
                e.1.idx.push(k);
            }
        }
        out.extend(by_part.into_values().map(|x| x.1));
    }
    out
}

/// "VLM3": the parts (u32 count, then each as in "VLM2"), the meshes (u32
/// count; per mesh shader, vertex count, index count, part and whether it
/// has bones, as u32; positions, UVs and indices as in "VLM1"; then, with
/// bones, four u16 bones and four u8 weights per vertex).
pub(super) fn encode(parts: &[(String, [f32; 3], usize)], meshes: &[Out]) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(b"VLM3");
    b.extend_from_slice(&(parts.len() as u32).to_le_bytes());
    for (label, c, n) in parts {
        b.extend_from_slice(&(label.len() as u16).to_le_bytes());
        b.extend_from_slice(label.as_bytes());
        while !b.len().is_multiple_of(4) {
            b.push(0);
        }
        for x in c {
            b.extend_from_slice(&x.to_le_bytes());
        }
        b.extend_from_slice(&(*n as u32).to_le_bytes());
    }
    b.extend_from_slice(&(meshes.len() as u32).to_le_bytes());
    for m in meshes {
        for v in [m.shader, m.pos.len() as u32, m.idx.len() as u32, m.part, m.skin.is_some() as u32] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        for p in m.pos.iter().flatten().chain(m.uv.iter().flatten()) {
            b.extend_from_slice(&p.to_le_bytes());
        }
        for i in &m.idx {
            b.extend_from_slice(&i.to_le_bytes());
        }
        if let Some((j, w)) = &m.skin {
            for x in j.iter().flatten() {
                b.extend_from_slice(&x.to_le_bytes());
            }
            b.extend(w.iter().flatten());
        }
    }
    b
}

/// A skeleton as the viewer takes it: per bone its name, parent (−1 for
/// none) and bind matrix (column-major, the bone's place in the model).
pub(super) fn skeleton_json(h: u32, bones: &[Bone]) -> String {
    let list: Vec<String> = bones
        .iter()
        .map(|b| format!("{{\"n\":{},\"p\":{},\"m\":[{}]}}", quote(&b.name), b.parent.map_or(-1, |p| p as i32), b.matrix.iter().map(|x| format!("{x:.5}")).collect::<Vec<_>>().join(",")))
        .collect();
    format!("{{\"h\":\"{h:08x}\",\"bones\":[{}]}}", list.join(","))
}

impl Lens {
    /// A model's skeleton, by `name` or hash `h`.
    pub(super) fn skeleton(&self, r: &Request) -> Reply {
        let h = match (r.get("name"), r.hex("h")) {
            (Some(n), _) => rage::joaat(n),
            (None, Some(h)) => h as u32,
            _ => return Reply::bad("name or h"),
        };
        let Some(bones) = self.skeleton_bones(h) else { return Reply::missing("no such model") };
        if bones.is_empty() {
            return Reply::missing("the model has no skeleton");
        }
        Reply::json(skeleton_json(h, &bones))
    }

    /// The bones of model `skel`'s skeleton as meshes and clips are matched
    /// against them: its own, and for a creature (a `SKEL_Head` but no face
    /// bones) the facial rig of the head beside it hung under `SKEL_Head`,
    /// so a face clip opens the mouth. The rig's places are in the head
    /// bone's frame (an eyeball's lands within a centimetre of the eye).
    pub(super) fn skeleton_bones(&self, skel: u32) -> Option<std::sync::Arc<Vec<Bone>>> {
        type Built = std::sync::Mutex<HashMap<u32, std::sync::Arc<Vec<Bone>>>>;
        static BUILT: std::sync::OnceLock<Built> = std::sync::OnceLock::new();
        let built = BUILT.get_or_init(Default::default);
        if let Some(b) = built.lock().unwrap().get(&skel) {
            return Some(b.clone());
        }
        let m = self.load(skel)?;
        let mut bones = m.drawable.bones.clone();
        let head = bones.iter().position(|b| b.name == "SKEL_Head");
        let faced = bones.iter().any(|b| b.name.starts_with("FACIAL_") || b.name.starts_with("FB_"));
        if let (Some(hi), false) = (head, faced)
            && let Some(rig) = self.head_beside(skel)
        {
            let (base, h) = (bones.len(), bones[hi].matrix);
            bones.extend(rig.drawable.bones.iter().map(|b| Bone { name: b.name.clone(), matrix: mul(&h, &b.matrix), parent: Some(b.parent.map_or(hi, |p| base + p as usize) as u16) }));
        }
        let bones = std::sync::Arc::new(bones);
        built.lock().unwrap().insert(skel, bones.clone());
        Some(bones)
    }

    /// The meshes of `m` on the skeleton of model `skel`, taken apart when
    /// `apart` (a vehicle).
    pub(super) fn skinned_mesh(&self, h: u32, m: &world::Model, skel: u32, apart: bool) -> Reply {
        let own = skel == h;
        let sk = if own { None } else { self.skeleton_bones(skel) };
        let bones = if own { &m.drawable.bones } else { sk.as_deref().unwrap_or(&m.drawable.bones) };
        let kin = if own || !m.drawable.bones.is_empty() { None } else { self.head_beside(skel) };
        let skins = skin(&m.drawable, bones, own, kin.as_ref().map_or(&[], |k| &k.drawable.bones));
        let (of, parts) = if apart { super::vehicles::take_apart(&m.drawable) } else { (Vec::new(), Vec::new()) };
        let meshes = split(&m.drawable, apart.then_some(of.as_slice()), &skins);
        Reply::bytes("application/octet-stream", encode(&parts, &meshes), false)
    }

    /// The head of the creature whose skeleton is model `skel`
    /// (`crm_dog_carolina_m_00` → `crm_dog_carolina_m_head_00` or
    /// `…_m_00_head_00`): its skeleton is the face's rig, which names the
    /// bones of the parts that have none (teeth).
    fn head_beside(&self, skel: u32) -> Option<std::sync::Arc<world::Model>> {
        let w = self.the_world()?;
        let name = w.index.names.get(&skel)?;
        let stem = name.trim_end_matches(|c: char| c.is_ascii_digit()).trim_end_matches('_');
        let mut heads: Vec<(&u32, &String)> = w.index.names.iter().filter(|(_, n)| n.starts_with(stem) && n[stem.len()..].contains("_head_")).collect();
        heads.sort_by_key(|(_, n)| n.len());
        heads.into_iter().filter_map(|(&h, _)| self.load(h)).find(|m| !m.drawable.bones.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eight_bones_become_the_four_heaviest_summing_to_255() {
        let m = Mesh { joints: vec![[3, 5, 3, 9, 1, 2, 0, 0]], weights: vec![[100, 60, 40, 30, 20, 5, 0, 0]], ..Mesh::default() };
        let (j, w) = four(&m, &[0, 10, 20, 30, 40, 50, 60, 70, 80, 90].map(Ride::Bone), &|_, _| 0);
        // Bone 3 twice (140), then 5 (60), 9 (30), 1 (20).
        assert_eq!(j[0], [30, 50, 90, 10]);
        assert_eq!(w[0].iter().map(|&x| x as u32).sum::<u32>(), 255);
        assert!(w[0][0] > w[0][1] && w[0][1] > w[0][2]);
        // A rigid vertex (one bone, no weights read) keeps it whole.
        let m = Mesh { joints: vec![[7, 0, 0, 0, 0, 0, 0, 0]], weights: vec![[255, 0, 0, 0, 0, 0, 0, 0]], ..Mesh::default() };
        assert_eq!(four(&m, &(0..10).map(Ride::Bone).collect::<Vec<_>>(), &|_, _| 0), (vec![[7, 0, 0, 0]], vec![[255, 0, 0, 0]]));
        // A bone the skeleton lacks goes to the one nearest the vertex, among
        // its body part's when its name gives one.
        let m = Mesh { joints: vec![[1, 2, 0, 0, 0, 0, 0, 0]], weights: vec![[200, 55, 0, 0, 0, 0, 0, 0]], positions: vec![[0.0, 0.0, 1.0]], ..Mesh::default() };
        let near = |p: [f32; 3], among: Option<&[u16]>| among.map_or(if p[2] > 0.5 { 9 } else { 0 }, |v| v[0]);
        assert_eq!(four(&m, &[Ride::Bone(0), Ride::Nearest, Ride::Among(vec![4, 5])], &near).0, vec![[9, 4, 0, 0]]);
    }

    #[test]
    fn a_seam_moves_as_one() {
        // Two meshes meet at (1, 0, 0): one side on bone 1, the other on 2.
        let mesh = |b: u16| Mesh { joints: vec![[0, 0, 0, 0, 0, 0, 0, 0]; 2], weights: vec![[255, 0, 0, 0, 0, 0, 0, 0]; 2], positions: vec![[1.0, 0.0, 0.0], [0.0, b as f32, 0.0]], ..Mesh::default() };
        let d = Drawable { meshes: vec![mesh(1), mesh(2)], ..Drawable::default() };
        let mut acc = vec![Some(vec![vec![(1, 255)], vec![(1, 255)]]), Some(vec![vec![(2, 255)], vec![(2, 255)]])];
        weld(&d, &mut acc);
        let a = acc[0].as_ref().unwrap();
        let b = acc[1].as_ref().unwrap();
        assert_eq!(top4(&a[0]), top4(&b[0]));
        assert_eq!(top4(&a[0]), ([1, 2, 0, 0], [127, 128, 0, 0]));
        // Elsewhere each keeps its own.
        assert_eq!(a[1], [(1, 255)]);
        assert_eq!(b[1], [(2, 255)]);
    }

    #[test]
    fn a_rig_in_the_head_frame_lands_where_the_head_puts_it() {
        // The head turned 90° about z (x → y) and placed at (0, 0, 1): a
        // point 0.1 along its x lies 0.1 along the model's y.
        let head = [0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0];
        let mut eye = [0.0; 16];
        for i in [0, 5, 10, 15] {
            eye[i] = 1.0;
        }
        eye[12] = 0.1;
        let m = mul(&head, &eye);
        assert!((m[12] - 0.0).abs() < 1e-6 && (m[13] - 0.1).abs() < 1e-6 && (m[14] - 1.0).abs() < 1e-6);
        assert_eq!(&m[..12], &head[..12]);
    }

    fn bones(names: &[&str]) -> Vec<Bone> {
        names.iter().map(|n| Bone { name: n.to_string(), matrix: [0.0; 16], parent: None }).collect()
    }

    #[test]
    fn cloth_bones_ride_the_body_part_they_name() {
        let sk = bones(&[
            "SKEL_ROOT",
            "SKEL_Pelvis",
            "SKEL_L_Thigh",
            "SKEL_L_Calf",
            "SKEL_R_Thigh",
            "SKEL_R_Calf",
            "SKEL_Spine5",
            "SKEL_Neck0",
            "SKEL_Neck1",
            "MH_L_Knee",
            "MH_L_KneeCrease",
            "BM_R_Deltoid",
            "BM_R_DeltoidMid",
            "SKEL_L_Foot",
            "SKEL_L_Toe0",
            "SKEL_L_Toe10",
        ]);
        let names = |v: Vec<u16>| v.into_iter().map(|i| sk[i as usize].name.as_str()).collect::<Vec<_>>();
        assert_eq!(names(rides("GM_L01_L_Calf02_CB", &sk)), ["SKEL_L_Calf"]);
        assert_eq!(names(rides("GM_L01_R_ThighButt_CB", &sk)), ["SKEL_R_Thigh"]);
        assert_eq!(names(rides("GM_Hood_C_Spine5_RB", &sk)), ["SKEL_Spine5"]);
        assert_eq!(names(rides("GM_U01_C_Pelvis_LF", &sk)), ["SKEL_Pelvis"]);
        assert_eq!(names(rides("GM_U01_C_Neck_CF", &sk)), ["SKEL_Neck0", "SKEL_Neck1"]);
        assert_eq!(names(rides("GM_L01_L_Knee_I", &sk)), ["MH_L_Knee"]);
        assert_eq!(names(rides("MH_Necklace_R_Delt_F", &sk)), ["BM_R_Deltoid", "BM_R_DeltoidMid"]);
        assert_eq!(names(rides("FlipFlop_L_Foot", &sk)), ["SKEL_L_Foot"]);
        assert_eq!(names(rides("MH_L_Toe", &sk)), ["SKEL_L_Toe0", "SKEL_L_Toe10"]);
        // A skirt's middle: either thigh.
        assert_eq!(names(rides("GM_U01_C_Thigh01_CF", &sk)), ["SKEL_L_Thigh", "SKEL_R_Thigh"]);
        // A face's rig rides the head.
        let sk = bones(&["SKEL_Neck1", "SKEL_Head"]);
        assert_eq!(rides("FACIAL_C_FacialRoot", &sk), [1]);
        assert_eq!(rides("FB_L_Brow_Out_000", &sk), [1]);
        // Names that give no part.
        assert!(rides("Watch_Buckle", &sk).is_empty());
        assert!(rides("HatBand3", &sk).is_empty());
    }

    #[test]
    fn the_nearest_bone_is_measured_to_its_length() {
        // A leg: hip at 0, knee 0.4 below, ankle 0.8 below; a helper bone
        // under the knee leads nowhere.
        let bone = |name: &str, parent: Option<u16>, z: f32| {
            let mut matrix = [0.0; 16];
            matrix[14] = z;
            Bone { name: name.into(), matrix, parent }
        };
        let sk = [bone("SKEL_L_Thigh", None, 0.0), bone("SKEL_L_Calf", Some(0), -0.4), bone("MH_L_Knee", Some(1), -0.4), bone("SKEL_L_Foot", Some(1), -0.8)];
        let l = lengths(&sk);
        assert_eq!(l.iter().map(|x| x.0).collect::<Vec<_>>(), [0, 1, 3]);
        let nearest = |p: [f32; 3]| l.iter().min_by(|a, b| to_segment(p, a.1, a.2).total_cmp(&to_segment(p, b.1, b.2))).unwrap().0;
        // Low on the thigh, beside it: nearer the knee's place than the
        // hip's, but on the thigh.
        assert_eq!(nearest([0.1, 0.0, -0.3]), 0);
        assert_eq!(nearest([0.1, 0.0, -0.5]), 1);
        assert_eq!(nearest([0.0, 0.0, -0.9]), 3);
    }
}

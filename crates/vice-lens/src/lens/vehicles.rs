//! Vice Lens: the vehicles, for the showroom. The game keeps a vehicle as
//! a body (`<name>_hi`, the detailed one; `_mid` and the bare name are
//! farther levels) with a skeleton naming its parts, tuning parts as
//! models of their own (`<name>_bonnet_02`, `<name>_spoiler_mu_04`, …),
//! and wheels apart: rims (`rim_<style>_<nnn>`) and tyres
//! (`tyre_<kind>_<nnn>`), both made a metre across and scaled to the
//! vehicle when drawn. Paint is not in the data either: the body's colour
//! map is made at runtime, so the showroom paints it.

use std::collections::BTreeMap;

use super::Lens;
use super::http::{Reply, Request, quote};

/// Words that name tuning parts, after the vehicle's name.
const PARTS: [&str; 20] =
    ["bonnet", "bumf", "bumr", "spoiler", "skirt", "exhaust", "exh", "roof", "grille", "wing", "splitter", "diffuser", "canard", "lights", "towing", "int", "mirror", "scoop", "livery", "bullbar"];

/// The wheel bones, in drawing order.
const WHEELS: [&str; 6] = ["wheel_lf", "wheel_rf", "wheel_lm", "wheel_rm", "wheel_lr", "wheel_rr"];

/// A base name of a model with levels: without `_hi` or `_mid`.
fn stem(n: &str) -> &str {
    n.strip_suffix("_hi").or_else(|| n.strip_suffix("_mid")).unwrap_or(n)
}

impl Lens {
    /// The detailed version of a model, by its base name: `<name>_hi`, else
    /// the name itself.
    fn detailed(&self, names: &std::collections::HashMap<u32, String>, base: &str) -> Option<u32> {
        let hi = rage::joaat(&format!("{base}_hi"));
        if names.contains_key(&hi) {
            return Some(hi);
        }
        let h = rage::joaat(base);
        names.contains_key(&h).then_some(h)
    }

    /// Every vehicle: a name with a `_hi` body and tuning parts named
    /// after it.
    pub(super) fn vehicles_list(&self) -> Reply {
        let Some(w) = self.the_world() else { return Reply::missing("no model index") };
        let names = &w.index.names;
        // Base name → its tuning parts (each once, whatever its levels).
        let mut parts: BTreeMap<&str, std::collections::HashSet<&str>> = BTreeMap::new();
        for n in names.values() {
            let n = stem(n);
            let at = PARTS.iter().filter_map(|p| n.find(&format!("_{p}_"))).min();
            if let Some(i) = at.filter(|&i| i > 0) {
                parts.entry(&n[..i]).or_default().insert(n);
            }
        }
        // A base that is another's part (`skivvy_canvascanopy`) is not a
        // vehicle of its own.
        let is_part = |base: &str| base.match_indices('_').any(|(i, _)| parts.contains_key(&base[..i]));
        let mut cars: Vec<String> = parts
            .iter()
            .filter(|(base, _)| names.contains_key(&rage::joaat(&format!("{base}_hi"))) && !base.starts_with("rim") && !base.starts_with("tyre") && !is_part(base))
            .map(|(base, p)| format!("{{\"name\":{},\"h\":\"{:08x}\",\"parts\":{}}}", quote(base), rage::joaat(&format!("{base}_hi")), p.len()))
            .collect();
        cars.sort();
        Reply::json(format!("{{\"items\":[{}]}}", cars.join(",")))
    }

    /// One vehicle: its body, where its wheels go, its tuning parts by
    /// kind, and the rims and tyres to choose from.
    pub(super) fn vehicle(&self, r: &Request) -> Reply {
        let Some(w) = self.the_world() else { return Reply::missing("no model index") };
        let Some(name) = r.get("name") else { return Reply::bad("name") };
        let names = &w.index.names;
        let Some(body) = self.detailed(names, name) else { return Reply::missing("no such vehicle") };
        let Some(m) = self.load(body) else { return Reply::missing("the body could not be read") };
        let d = &m.drawable;
        let wheels: Vec<String> = WHEELS
            .iter()
            .filter_map(|wn| d.bones.iter().find(|b| b.name == *wn))
            .map(|b| format!("{{\"bone\":{},\"at\":[{},{},{}]}}", quote(&b.name), b.matrix[12], b.matrix[13], b.matrix[14]))
            .collect();
        // Tuning parts, by kind, each once (its detailed version).
        let prefix = format!("{name}_");
        let mut kinds: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        let mut seen = std::collections::HashSet::new();
        for n in names.values() {
            let n = stem(n);
            let Some(rest) = n.strip_prefix(&prefix) else { continue };
            let Some(kind) = PARTS.iter().find(|p| rest.starts_with(**p) && rest[p.len()..].starts_with('_')) else { continue };
            if !seen.insert(n.to_string()) {
                continue;
            }
            if let Some(h) = self.detailed(names, n) {
                kinds.entry(kind).or_default().push(format!("{{\"name\":{},\"h\":\"{h:08x}\"}}", quote(n)));
            }
        }
        let parts: Vec<String> = kinds
            .iter_mut()
            .map(|(k, v)| {
                v.sort();
                format!("{}:[{}]", quote(k), v.join(","))
            })
            .collect();
        // Every bone's place in the model, for the tuning parts (each built
        // around the bone it hangs on).
        let skeleton: Vec<String> = d.bones.iter().map(|b| format!("{}:[{}]", quote(&b.name), b.matrix.iter().map(|x| format!("{x:.5}")).collect::<Vec<_>>().join(","))).collect();
        Reply::json(format!(
            "{{\"name\":{},\"body\":\"{body:08x}\",\"min\":[{},{},{}],\"max\":[{},{},{}],\"wheels\":[{}],\"bones\":{},\"skeleton\":{{{}}},\"parts\":{{{}}}}}",
            quote(name),
            d.min[0],
            d.min[1],
            d.min[2],
            d.max[0],
            d.max[1],
            d.max[2],
            wheels.join(","),
            d.bones.len(),
            skeleton.join(","),
            parts.join(",")
        ))
    }

    /// Rims and tyres by style: `rim_<style>_<nnn>`, `tyre_<kind>_<nnn>`,
    /// each with its detailed version.
    pub(super) fn wheels_list(&self) -> Reply {
        let Some(w) = self.the_world() else { return Reply::missing("no model index") };
        let names = &w.index.names;
        let mut sets: BTreeMap<(&str, String), Vec<String>> = BTreeMap::new();
        let mut seen = std::collections::HashSet::new();
        for n in names.values() {
            let n = stem(n);
            for what in ["rim", "tyre"] {
                let Some(rest) = n.strip_prefix(what).and_then(|r| r.strip_prefix('_')) else { continue };
                let Some((style, num)) = rest.rsplit_once('_') else { continue };
                if num.is_empty() || !num.bytes().all(|c| c.is_ascii_digit()) || !seen.insert(n.to_string()) {
                    continue;
                }
                if let Some(h) = self.detailed(names, n) {
                    sets.entry((what, style.to_string())).or_default().push(format!("{{\"name\":{},\"h\":\"{h:08x}\"}}", quote(n)));
                }
            }
        }
        let mut list = |what: &str| -> String {
            sets.iter_mut()
                .filter(|((w, _), _)| *w == what)
                .map(|((_, style), v)| {
                    v.sort();
                    format!("{}:[{}]", quote(style), v.join(","))
                })
                .collect::<Vec<_>>()
                .join(",")
        };
        let (rims, tyres) = (list("rim"), list("tyre"));
        Reply::json(format!("{{\"rims\":{{{rims}}},\"tyres\":{{{tyres}}}}}"))
    }
}

/// The parts a vehicle can be taken apart into, by the bone each piece of
/// the body sits nearest (its name, what to call it). Pieces are the
/// body's connected pieces of surface (a door, a seat, a panel).
const PART_BONES: [(&str, &str); 30] = [
    ("steeringwheel", "Steering wheel"),
    ("seat_dside_f", "Driver's seat"),
    ("seat_pside_f", "Passenger seat"),
    ("seat_dside_r", "Rear seat, left"),
    ("seat_pside_r", "Rear seat, right"),
    ("door_dside_f", "Door, front left"),
    ("door_pside_f", "Door, front right"),
    ("door_dside_r", "Door, rear left"),
    ("door_pside_r", "Door, rear right"),
    ("bonnet", "Bonnet"),
    ("boot", "Boot"),
    ("bumper_f", "Front bumper"),
    ("bumper_r", "Rear bumper"),
    ("wing_lf", "Wing, left"),
    ("wing_rf", "Wing, right"),
    ("engine", "Engine"),
    ("exhaust", "Exhaust, left"),
    ("exhaust_2", "Exhaust, right"),
    ("wheel_lf", "Front left hub"),
    ("wheel_rf", "Front right hub"),
    ("wheel_lr", "Rear left hub"),
    ("wheel_rr", "Rear right hub"),
    ("dashboard", "Dashboard"),
    ("headlight_l", "Headlight, left"),
    ("headlight_r", "Headlight, right"),
    ("taillight_l", "Tail light, left"),
    ("taillight_r", "Tail light, right"),
    ("roof", "Roof"),
    ("chassis", "Chassis"),
    ("interior", "Interior"),
];

/// The part a bone belongs to: the first of it and its parents that names
/// a part (a door's window and card go with the door), but not the chassis
/// (the body shell and what is bolted to it are told apart by place).
fn bone_part(d: &rage::drawable::Drawable, mut b: usize) -> Option<&'static str> {
    for _ in 0..64 {
        let bone = d.bones.get(b)?;
        if bone.name == "chassis" || bone.name == "bodyshell" {
            return None;
        }
        if let Some((_, label)) = PART_BONES.iter().find(|(n, _)| *n == bone.name) {
            return Some(label);
        }
        b = bone.parent? as usize;
    }
    None
}

/// Parts: label, centre, triangles.
pub(super) type Parts = Vec<(String, [f32; 3], usize)>;

/// A drawable taken apart: per mesh, per triangle, the part it belongs to,
/// and the parts. Without a skeleton, one part.
pub(super) fn take_apart(d: &rage::drawable::Drawable) -> (Vec<Vec<u16>>, Parts) {
    let bones: Vec<(&str, [f32; 3])> = PART_BONES.iter().filter_map(|(n, label)| d.bones.iter().find(|b| b.name == *n).map(|b| (*label, [b.matrix[12], b.matrix[13], b.matrix[14]]))).collect();
    let size = [0, 1, 2].map(|k| d.max[k] - d.min[k]);
    let big = size.iter().cloned().fold(0.0f32, f32::max) * 0.55;
    let mut labels: Vec<String> = Vec::new();
    let mut sums: Vec<([f64; 3], usize)> = Vec::new();
    let mut part_of = |label: &str, c: [f32; 3], tris: usize| -> u16 {
        let i = labels.iter().position(|l| l == label).unwrap_or_else(|| {
            labels.push(label.to_string());
            sums.push(([0.0; 3], 0));
            labels.len() - 1
        });
        for (s, x) in sums[i].0.iter_mut().zip(c) {
            *s += x as f64 * tris as f64;
        }
        sums[i].1 += tris;
        i as u16
    };
    let mut out = Vec::new();
    for (mi, m) in d.meshes.iter().enumerate() {
        let tris = m.indices.len() / 3;
        let glass = d.buckets.get(m.shader) == Some(&rage::drawable::Bucket::Alpha);
        // Pieces: triangles joined where they share a corner (corners at
        // one place count as one, across seams).
        let mut id: std::collections::HashMap<[i32; 3], u32> = std::collections::HashMap::new();
        let corner: Vec<u32> = m
            .positions
            .iter()
            .map(|p| {
                let q = p.map(|x| (x * 1000.0).round() as i32);
                let n = id.len() as u32;
                *id.entry(q).or_insert(n)
            })
            .collect();
        let mut up: Vec<u32> = (0..id.len() as u32).collect();
        fn find(up: &mut [u32], mut x: u32) -> u32 {
            while up[x as usize] != x {
                up[x as usize] = up[up[x as usize] as usize];
                x = up[x as usize];
            }
            x
        }
        for t in m.indices.as_chunks::<3>().0 {
            let a = find(&mut up, corner[t[0] as usize]);
            for &v in &t[1..] {
                let b = find(&mut up, corner[v as usize]);
                if a != b {
                    up[b as usize] = a;
                }
            }
        }
        // Each piece's bounds.
        let mut pieces: std::collections::HashMap<u32, ([f32; 3], [f32; 3], usize)> = std::collections::HashMap::new();
        let roots: Vec<u32> = m.indices.as_chunks::<3>().0.iter().map(|t| find(&mut up, corner[t[0] as usize])).collect();
        for (t, &r) in m.indices.as_chunks::<3>().0.iter().zip(&roots) {
            let e = pieces.entry(r).or_insert(([f32::MAX; 3], [f32::MIN; 3], 0));
            for &v in t {
                let p = m.positions[v as usize];
                for ((lo, hi), x) in e.0.iter_mut().zip(e.1.iter_mut()).zip(p) {
                    *lo = lo.min(x);
                    *hi = hi.max(x);
                }
            }
            e.2 += 1;
        }
        let mut label_of: std::collections::HashMap<u32, u16> = std::collections::HashMap::new();
        // The bone each piece follows (vehicles bind every vertex to one):
        // that of its first triangle's first corner.
        let mut piece_bone: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
        if !m.joints.is_empty() && m.palette.is_empty() {
            for (t, &r) in m.indices.as_chunks::<3>().0.iter().zip(&roots) {
                piece_bone.entry(r).or_insert_with(|| {
                    let v = t[0] as usize;
                    let k = (0..8).max_by_key(|&k| m.weights[v][k]).unwrap_or(0);
                    m.joints[v][k] as usize
                });
            }
        }
        for (&r, (lo, hi, n)) in &pieces {
            let c = [0, 1, 2].map(|k| (lo[k] + hi[k]) / 2.0);
            let span = [0, 1, 2].map(|k| hi[k] - lo[k]).into_iter().fold(0.0f32, f32::max);
            let by_bone = piece_bone.get(&r).and_then(|&b| bone_part(d, b));
            let label = if glass {
                "Glass"
            } else if let Some(l) = by_bone {
                l
            } else if bones.is_empty() {
                "Model"
            } else if span > big {
                "Body shell"
            } else {
                bones
                    .iter()
                    .map(|(l, b)| (*l, ((c[0] - b[0]).powi(2) + (c[1] - b[1]).powi(2) + (c[2] - b[2]).powi(2)).sqrt()))
                    .filter(|x| x.1 < 0.9)
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                    .map_or("Body", |x| x.0)
            };
            label_of.insert(r, part_of(label, c, *n));
        }
        let _ = mi;
        out.push(roots.iter().map(|r| label_of[r]).collect());
        let _ = tris;
    }
    let parts = labels.into_iter().zip(sums).map(|(l, (s, n))| (l, s.map(|x| (x / n.max(1) as f64) as f32), n)).collect();
    (out, parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_of_a_model_share_its_base_name() {
        assert_eq!(stem("infernus2_hi"), "infernus2");
        assert_eq!(stem("rim_race_005_mid"), "rim_race_005");
        assert_eq!(stem("infernus2"), "infernus2");
    }
}

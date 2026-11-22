//! Vice Lens: a model's textures slot by slot (every texture its shaders
//! name, not only the colour map), the models that are variants of it, and
//! the model as glTF (`.glb`) to take into Blender or any other tool.

use std::collections::HashMap;

use rage::drawable::{Bucket, TexRef};

use super::http::{Reply, Request, quote};
use super::{Lens, encode_png};
use crate::world;

/// Texture parameter names the shaders use, as far as known: the name and
/// what it holds. Parameters not listed show as their hash.
const PARAM_NAMES: [(&str, &str); 22] = [
    ("diffusetex", "colour"),
    ("diffusesampler", "colour"),
    ("diffusetex2", "second colour"),
    ("diffusetexture_layer0", "ground layer 1"),
    ("diffusetexture_layer1", "ground layer 2"),
    ("diffusetexture_layer2", "ground layer 3"),
    ("diffusetexture_layer3", "ground layer 4"),
    ("bumptexture_layer0", "layer 1 normals"),
    ("bumptexture_layer1", "layer 2 normals"),
    ("bumptexture_layer2", "layer 3 normals"),
    ("bumptexture_layer3", "layer 4 normals"),
    ("lookuptexture", "layer blend map"),
    ("tinttexture", "tint"),
    ("bumptex", "normals"),
    ("bumpsampler", "normals"),
    ("speculartex", "specular"),
    ("specsampler", "specular"),
    ("detailsampler", "detail"),
    ("detailtex", "detail"),
    ("tintpalettetex", "tint palette"),
    ("emissivetex", "glow"),
    ("dirtsampler", "dirt"),
];

/// The ground shader's three colour maps, whose names are not known.
const GROUND: [u32; 3] = [0x3463_94ea, 0x7d4f_12e2, 0x7485_dafb];

fn param_name(hash: u32) -> (String, &'static str) {
    if let Some((n, what)) = PARAM_NAMES.iter().find(|(n, _)| rage::joaat(n) == hash) {
        return (n.to_string(), what);
    }
    match GROUND.iter().position(|&g| g == hash) {
        Some(0) => (format!("{hash:08x}"), "ground colour 1"),
        Some(1) => (format!("{hash:08x}"), "ground colour 2"),
        Some(2) => (format!("{hash:08x}"), "ground colour 3"),
        _ => (format!("{hash:08x}"), ""),
    }
}

fn bucket_name(b: Bucket) -> &'static str {
    match b {
        Bucket::Opaque => "opaque",
        Bucket::Alpha => "glass",
        Bucket::Decal => "decal",
        Bucket::Cutout => "cut-out",
        Bucket::Water => "water",
    }
}

/// Halve an RGBA8 image until it is at most `max` pixels across.
pub(super) fn shrink(mut px: Vec<u8>, mut w: u32, mut h: u32, max: u32) -> (Vec<u8>, u32, u32) {
    while w.max(h) > max && w > 1 && h > 1 {
        let (nw, nh) = (w / 2, h / 2);
        let mut out = vec![0u8; (nw * nh * 4) as usize];
        for y in 0..nh {
            for x in 0..nw {
                for c in 0..4 {
                    let s = |dx: u32, dy: u32| px[(((y * 2 + dy) * w + x * 2 + dx) * 4 + c) as usize] as u32;
                    out[((y * nw + x) * 4 + c) as usize] = ((s(0, 0) + s(1, 0) + s(0, 1) + s(1, 1)) / 4) as u8;
                }
            }
        }
        (w, h, px) = (nw, nh, out);
    }
    (px, w, h)
}

/// The colour map a shader shows by default: full size when it decodes,
/// else its small copy.
fn shown_rgba(m: &world::Model, si: usize) -> Option<(Vec<u8>, u32, u32)> {
    let t = m.textures.get(si)?;
    if let Some((dxgi, w, h, data)) = &t.pixels
        && let Some(rgba) = crate::assets::decode(data, *dxgi, *w, *h)
    {
        return Some((rgba, *w, *h));
    }
    t.small.as_ref().map(|(w, h, data)| (data[..(w * h * 4) as usize].to_vec(), *w, *h))
}

/// Which parameter holds the colour map a shader shows by default.
fn shown_param(m: &world::Model, si: usize) -> Option<u32> {
    let t = m.textures.get(si)?;
    let params = m.drawable.params.get(si)?;
    let albedo = m.drawable.shaders.get(si)?;
    match t.key {
        Some(k) => params.iter().find(|(_, r)| *r == TexRef::Key(k)).map(|p| p.0),
        None if t.small.is_some() => params.iter().find(|(_, r)| matches!(r, TexRef::Embedded(_)) && albedo.contains(r)).map(|p| p.0),
        None => None,
    }
}

impl Lens {
    /// The pixels of texture parameter `p` of shader `si` of model `h`.
    fn slot_rgba(&self, h: u32, m: &world::Model, si: usize, p: u32) -> Option<(Vec<u8>, u32, u32)> {
        let (_, t) = m.drawable.params.get(si)?.iter().find(|(ph, _)| *ph == p)?;
        match t {
            TexRef::Key(k) => {
                let tx = self.textures.get_or_init(|| world::Textures::open(&self.game));
                let (dxgi, w, ht, data) = tx.get(*k)?;
                Some((crate::assets::decode(&data, dxgi, w, ht)?, w, ht))
            }
            TexRef::Embedded(at) => {
                let w = self.the_world()?;
                let &(a, off, _) = w.index.by_hash.get(&h)?;
                let res = self.archive(&w.index.archives[a])?.resource_at(off).ok()?;
                let d = &m.drawable;
                let i = d.embedded.iter().position(|t| t.at == *at)?;
                let allocs = rage::txd::allocations(&d.embedded, res.phys.len());
                let t = &d.embedded[i];
                let lin = rage::txd::top_mip(&res.phys, t, allocs[i])?;
                Some((crate::assets::decode(&lin, t.dxgi, t.width, t.height)?, t.width, t.height))
            }
        }
    }

    /// Every texture of every shader of a model: parameter, name, size,
    /// format, whether it decodes; and which one the viewer shows.
    pub(super) fn model_slots(&self, r: &Request) -> Reply {
        let Some(h) = r.hex("h").map(|h| h as u32) else { return Reply::bad("h") };
        let Some(m) = self.load(h) else { return Reply::missing("the model could not be read") };
        let tx = self.textures.get_or_init(|| world::Textures::open(&self.game));
        let d = &m.drawable;
        let shaders: Vec<String> = d
            .params
            .iter()
            .enumerate()
            .map(|(si, params)| {
                let slots: Vec<String> = params
                    .iter()
                    .map(|(p, t)| {
                        let (name, what) = param_name(*p);
                        let (w, ht, dxgi, ok, from) = match t {
                            TexRef::Key(k) => match tx.info(*k) {
                                Some((w, ht, dxgi, ok)) => (w, ht, dxgi, ok, "pack"),
                                None => (0, 0, 0, false, "missing"),
                            },
                            TexRef::Embedded(at) => match d.embedded.iter().find(|e| e.at == *at) {
                                Some(e) => (e.width, e.height, e.dxgi, true, "model"),
                                None => (0, 0, 0, false, "missing"),
                            },
                        };
                        format!(
                            "{{\"p\":\"{p:08x}\",\"name\":{},\"what\":{},\"w\":{w},\"h\":{ht},\"format\":{},\"ok\":{ok},\"from\":\"{from}\",\"colour\":{}}}",
                            quote(&name),
                            quote(what),
                            quote(super::media::dxgi_name(dxgi)),
                            d.shaders.get(si).is_some_and(|a| a.contains(t))
                        )
                    })
                    .collect();
                let meshes = d.meshes.iter().filter(|me| me.shader == si).count();
                let shown = shown_param(&m, si).map_or("null".into(), |p| format!("\"{p:08x}\""));
                let bucket = bucket_name(d.buckets.get(si).copied().unwrap_or_default());
                format!("{{\"shader\":{si},\"meshes\":{meshes},\"bucket\":\"{bucket}\",\"shown\":{shown},\"slots\":[{}]}}", slots.join(","))
            })
            .collect();
        Reply::json(format!("{{\"shaders\":[{}]}}", shaders.join(",")))
    }

    /// One texture of a model as PNG: parameter `p` of shader `s`, at most
    /// `max` pixels across.
    pub(super) fn model_slot(&self, r: &Request) -> Reply {
        let (Some(h), Some(si), Some(p)) = (r.hex("h").map(|h| h as u32), r.num::<usize>("s"), r.hex("p").map(|p| p as u32)) else { return Reply::bad("h, s, p") };
        let Some(m) = self.load(h) else { return Reply::missing("the model could not be read") };
        let Some((px, w, ht)) = self.slot_rgba(h, &m, si, p) else { return Reply::missing("this texture does not decode (BCPack, or not in the packs)") };
        let (px, w, ht) = shrink(px, w, ht, r.num::<u32>("max").unwrap_or(u32::MAX));
        Reply::bytes("image/png", encode_png(&px, w, ht), true)
    }

    /// The models whose names make them variants of this one (see
    /// `variants`), this one included.
    pub(super) fn model_family(&self, r: &Request) -> Reply {
        let Some(w) = self.the_world() else { return Reply::missing("no model index") };
        let Some(h) = r.hex("h").map(|h| h as u32) else { return Reply::bad("h") };
        let Some(name) = w.index.names.get(&h) else { return Reply::json("{\"items\":[]}".into()) };
        let mut v: Vec<(u32, &String)> = w.index.names.iter().filter(|(_, n)| super::variants(name, n)).map(|(k, n)| (*k, n)).collect();
        if !v.is_empty() {
            v.push((h, name));
        }
        v.sort_by(|a, b| a.1.cmp(b.1));
        let items: Vec<String> = v.iter().map(|(k, n)| format!("{{\"h\":\"{k:08x}\",\"name\":{},\"placed\":{}}}", quote(n), w.by_model.get(k).map_or(0, Vec::len))).collect();
        Reply::json(format!("{{\"items\":[{}]}}", items.join(",")))
    }

    /// The model as binary glTF, with the colour maps the viewer shows; `t`
    /// lists the ones chosen instead (`shader:parameter,…`).
    pub(super) fn model_export(&self, r: &Request) -> Reply {
        let Some(h) = r.hex("h").map(|h| h as u32) else { return Reply::bad("h") };
        let Some(m) = self.load(h) else { return Reply::missing("the model could not be read") };
        let chosen: HashMap<usize, u32> = r
            .get("t")
            .unwrap_or("")
            .split(',')
            .filter_map(|x| {
                let (a, b) = x.split_once(':')?;
                Some((a.parse().ok()?, u32::from_str_radix(b, 16).ok()?))
            })
            .collect();
        let name = self.the_world().map_or(format!("{h:08x}"), |w| w.name(h));
        let body = glb(&m, &name, |si| match chosen.get(&si) {
            Some(&p) => self.slot_rgba(h, &m, si, p),
            None => shown_rgba(&m, si),
        });
        let mut reply = Reply::bytes("model/gltf-binary", body, false);
        reply.extra.push(("Content-Disposition", format!("attachment; filename=\"{name}.glb\"")));
        reply
    }
}

/// The binary buffer of a glTF file and the views and accessors into it.
#[derive(Default)]
struct Gltf {
    bin: Vec<u8>,
    views: Vec<String>,
    accessors: Vec<String>,
}

impl Gltf {
    fn view(&mut self, bytes: &[u8], target: Option<u32>) -> usize {
        while !self.bin.len().is_multiple_of(4) {
            self.bin.push(0);
        }
        let target = target.map_or(String::new(), |t| format!(",\"target\":{t}"));
        self.views.push(format!("{{\"buffer\":0,\"byteOffset\":{},\"byteLength\":{}{target}}}", self.bin.len(), bytes.len()));
        self.bin.extend_from_slice(bytes);
        self.views.len() - 1
    }

    fn accessor(&mut self, bytes: &[u8], target: u32, component: u32, count: usize, kind: &str, bounds: &str) -> usize {
        let v = self.view(bytes, Some(target));
        self.accessors.push(format!("{{\"bufferView\":{v},\"componentType\":{component},\"count\":{count},\"type\":\"{kind}\"{bounds}}}"));
        self.accessors.len() - 1
    }
}

/// A model as binary glTF 2.0: one mesh with a primitive per piece, a
/// material per shader with its colour map (`image`) as PNG. The game's
/// Z-up metres become glTF's Y-up metres.
fn glb(m: &world::Model, name: &str, image: impl Fn(usize) -> Option<(Vec<u8>, u32, u32)>) -> Vec<u8> {
    let d = &m.drawable;
    let mut g = Gltf::default();
    let shaders = d.params.len().max(d.meshes.iter().map(|me| me.shader + 1).max().unwrap_or(0));
    let (mut materials, mut textures, mut images) = (Vec::new(), Vec::new(), Vec::new());
    for si in 0..shaders {
        let tex = image(si).map(|(px, w, h)| {
            let v = g.view(&encode_png(&px, w, h), None);
            images.push(format!("{{\"bufferView\":{v},\"mimeType\":\"image/png\"}}"));
            textures.push(format!("{{\"sampler\":0,\"source\":{}}}", images.len() - 1));
            textures.len() - 1
        });
        let pbr = match tex {
            Some(t) => format!("\"baseColorTexture\":{{\"index\":{t}}},\"metallicFactor\":0,\"roughnessFactor\":0.9"),
            None => "\"baseColorFactor\":[0.62,0.65,0.7,1],\"metallicFactor\":0,\"roughnessFactor\":0.9".into(),
        };
        let alpha = match d.buckets.get(si).copied().unwrap_or_default() {
            Bucket::Cutout => ",\"alphaMode\":\"MASK\",\"alphaCutoff\":0.3",
            Bucket::Alpha | Bucket::Decal | Bucket::Water => ",\"alphaMode\":\"BLEND\"",
            Bucket::Opaque => "",
        };
        materials.push(format!("{{\"name\":\"shader {si}\",\"pbrMetallicRoughness\":{{{pbr}}},\"doubleSided\":true{alpha}}}"));
    }
    let mut prims = Vec::new();
    for me in d.meshes.iter().filter(|me| !me.indices.is_empty() && !me.positions.is_empty()) {
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        let mut pos = Vec::with_capacity(me.positions.len() * 12);
        for p in &me.positions {
            // (+ 0.0 turns -0 into 0.)
            let q = [p[0], p[2], -p[1] + 0.0];
            for c in 0..3 {
                lo[c] = lo[c].min(q[c]);
                hi[c] = hi[c].max(q[c]);
                pos.extend_from_slice(&q[c].to_le_bytes());
            }
        }
        // A terrain blend shader tiles its layers by the second UV set.
        let layered = d.layered.get(me.shader) == Some(&true) && me.uvs1.len() == me.positions.len();
        let mut uv = Vec::with_capacity(me.positions.len() * 8);
        for i in 0..me.positions.len() {
            let t = if layered { me.uvs1[i] } else { me.uvs.get(i).copied().unwrap_or([0.0, 0.0]) };
            uv.extend_from_slice(&t[0].to_le_bytes());
            uv.extend_from_slice(&t[1].to_le_bytes());
        }
        let idx: Vec<u8> = me.indices.iter().flat_map(|i| i.to_le_bytes()).collect();
        let n = me.positions.len();
        let bounds = format!(",\"min\":[{},{},{}],\"max\":[{},{},{}]", lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]);
        let a_pos = g.accessor(&pos, 34962, 5126, n, "VEC3", &bounds);
        let a_uv = g.accessor(&uv, 34962, 5126, n, "VEC2", "");
        let a_idx = g.accessor(&idx, 34963, 5125, me.indices.len(), "SCALAR", "");
        prims.push(format!("{{\"attributes\":{{\"POSITION\":{a_pos},\"TEXCOORD_0\":{a_uv}}},\"indices\":{a_idx},\"material\":{}}}", me.shader.min(shaders.saturating_sub(1))));
    }
    while !g.bin.len().is_multiple_of(4) {
        g.bin.push(0);
    }
    let mut json = format!(
        "{{\"asset\":{{\"version\":\"2.0\",\"generator\":\"Vice Lens\"}},\"scene\":0,\"scenes\":[{{\"nodes\":[0]}}],\"nodes\":[{{\"mesh\":0,\"name\":{n}}}],\"meshes\":[{{\"name\":{n},\"primitives\":[{}]}}],\"materials\":[{}],\"textures\":[{}],\"images\":[{}],\"samplers\":[{{\"wrapS\":10497,\"wrapT\":10497,\"minFilter\":9987,\"magFilter\":9729}}],\"accessors\":[{}],\"bufferViews\":[{}],\"buffers\":[{{\"byteLength\":{}}}]}}",
        prims.join(","),
        materials.join(","),
        textures.join(","),
        images.join(","),
        g.accessors.join(","),
        g.views.join(","),
        g.bin.len(),
        n = quote(name),
    )
    .into_bytes();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let total = 12 + 8 + json.len() + 8 + g.bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json);
    out.extend_from_slice(&(g.bin.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&g.bin);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rage::drawable::{Drawable, Mesh};

    #[test]
    fn known_parameter_names_hash_to_what_the_shaders_hold() {
        assert_eq!(param_name(0xc8e8_c282).0, "diffusetex");
        assert_eq!(param_name(0xce7b_968e).0, "diffusetexture_layer0");
        assert_eq!(param_name(0x1e77_bf4c).0, "bumptex");
        assert_eq!(param_name(0xc075_f9b6).0, "lookuptexture");
        assert_eq!(param_name(0x3fb7_964f).0, "tinttexture");
        assert_eq!(param_name(0x1234_5678), ("12345678".to_string(), ""));
    }

    #[test]
    fn a_model_writes_as_valid_binary_gltf() {
        let mesh = Mesh { positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 2.0, 3.0]], uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]], indices: vec![0, 1, 2], ..Mesh::default() };
        let d = Drawable { meshes: vec![mesh], shaders: vec![Vec::new()], params: vec![Vec::new()], buckets: vec![Bucket::Cutout], layered: vec![false], ..Drawable::default() };
        let m = world::Model { drawable: d, vertices: Vec::new(), textures: Vec::new(), sphere: [0.0; 4] };
        let b = glb(&m, "box \"1\"", |_| Some((vec![255; 4 * 4], 2, 2)));
        assert_eq!(&b[..4], b"glTF");
        assert_eq!(u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize, b.len());
        let jl = u32::from_le_bytes(b[12..16].try_into().unwrap()) as usize;
        assert_eq!(&b[16..20], b"JSON");
        let json = std::str::from_utf8(&b[20..20 + jl]).unwrap();
        assert!(json.contains("\"alphaMode\":\"MASK\"") && json.contains("\"name\":\"box \\\"1\\\"\""));
        // Z-up (0, 2, 3) is Y-up (0, 3, -2): the bounds say so.
        assert!(json.contains("\"min\":[0,0,-2]") && json.contains("\"max\":[1,3,0]"), "{json}");
        let bl = u32::from_le_bytes(b[20 + jl..24 + jl].try_into().unwrap()) as usize;
        assert_eq!(&b[24 + jl..28 + jl], b"BIN\0");
        assert_eq!(28 + jl + bl, b.len());
        assert!(jl.is_multiple_of(4) && bl.is_multiple_of(4));
    }
}

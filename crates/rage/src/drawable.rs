//! Drawables (`.#dr`, and the drawable inside a fragment `.#ft`): RAGE 9
//! meshlet geometry with quantised positions.
//!
//! Layout (virtual segment, from the drawable base; a fragment keeps its
//! drawable at the pointer at +0x18):
//! - `+0x10` shader group: `+0x08` embedded texture dictionary, `+0x10`
//!   shaders, `+0x18` u16 count.
//! - `+0x20` bounding sphere (centre, radius), `+0x30` box min, `+0x40` box max.
//! - `+0x50` LOD 0: models pointer, u16 count. A model: `+0x08` geometries,
//!   `+0x10` u16 count, `+0x20` u16 shader index per geometry.
//! - Geometry: `+0x60` = 0x1601, `+0x68` vertex buffer (physical), `+0x58`
//!   its size in 32-bit words, `+0x40` link → meshlet object (`+0x08` words,
//!   `+0x18` meshlet buffer, physical).
//! - Vertex buffer: stream headers (`(offset/2) << 8 | type`), one per slot
//!   up to `+0x5c`, zero for an empty slot; stream 0
//!   holds positions as 8 bytes (two 24-bit slots, one 16-bit) scaled by the
//!   floats at `+0x70` and offset by those at `+0x60`, per slot; `+0x6c`
//!   says which model axis each slot holds (see `slot_axes`); a type 0x63
//!   stream 0 holds plain f32 positions instead. UV0 is a type 0x28 stream
//!   (u16, 1/8192) or the first 0x42 stream after the positions (two f32).
//!   Slots 6 and 7 hold bone weights (type 0x26, four bytes each, eight
//!   summing to 255), slots 8 and 9 the bones (see `JOINT_SIZE`); without
//!   weights, slot 8 alone binds each vertex to one bone (vehicle pieces).
//!   A model's `+0x28` points at its bone palette, `+0x14` (u16) its size:
//!   the sorted `joaat` of bone names for skinned characters and animals;
//!   on vehicles a bit set of the skeleton's bones, the indices being the
//!   skeleton's own.
//! - Meshlet buffer: u32 meshlet count (low 16 bits), u32 triangles, u32
//!   vertex references; 16 bytes per meshlet (`+4` first reference, `+6`
//!   first triangle); triangles as three 7-bit local references; u16 remap.
//! - Shader (0x40 bytes): `+0x00` u32 effect hash, then `meta`; `+0x10`
//!   texture references, `+0x20` parameter info (byte 1: texture count,
//!   byte 4: parameter count, parameters from +8 as `u32 hash, u32 data`;
//!   data & 3 == 0 means texture index (data >> 2) & 255); `+0x39` render
//!   bucket (see `Bucket`), `+0x3c` the bucket as a bit mask.
//!   A texture reference's `+0x30` points at 16 bytes: an embedded texture
//!   pointer, or zero and a 64-bit TTPF key.

use crate::rsc::{pptr, vptr};
use crate::{le, txd};

/// Shader parameters that hold an albedo map, best first: `diffusetex`,
/// `diffusesampler`, `diffusetex2`; the terrain blend shaders'
/// `diffusetexture_layer0..3`; and the three sRGB layers of the 12-texture
/// ground shader (names unknown, told apart by their sRGB formats).
const ALBEDO: [u32; 10] = [0xc8e8_c282, 0xf1fe_2b71, 0x0df4_7048, 0xce7b_968e, 0xd883_aa9e, 0x32b9_df09, 0x3d10_f3b7, 0x3463_94ea, 0x7d4f_12e2, 0x7485_dafb];

#[derive(Clone, Debug, PartialEq)]
pub enum TexRef {
    /// A texture in the drawable's own dictionary (its struct offset).
    Embedded(usize),
    /// A texture in one of the TTPF packs.
    Key(u64),
}

#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    /// The second UV set, when the mesh has one (empty otherwise): the
    /// terrain blend shader tiles its layers with it, UV0 placing the blend
    /// map over the whole piece.
    pub uvs1: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
    /// Index into `Drawable::shaders`.
    pub shader: usize,
    /// Vertex streams: (type, byte offset in the vertex buffer).
    pub streams: Vec<(u8, usize)>,
    /// Vertex buffer offset in the physical segment.
    pub vb: usize,
    /// Per vertex, up to eight bones it follows and their weights (out of
    /// 255, largest first not guaranteed): empty when the mesh follows no
    /// bone. The bones are indices into `palette` when it is not empty,
    /// else into the skeleton (vehicles, whose rigid pieces each follow one).
    pub joints: Vec<[u16; 8]>,
    pub weights: Vec<[u8; 8]>,
    /// The model's bone palette: the `joaat` of each bone's name, sorted.
    /// Characters and animals are skinned against a skeleton held in
    /// another model, matched by these names.
    pub palette: Vec<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct Drawable {
    pub name: String,
    pub sphere: [f32; 4],
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub meshes: Vec<Mesh>,
    /// Per shader, its albedo maps, best first (a renderer takes the first
    /// it can decode; terrain layers may use the console-only BCPack codec).
    pub shaders: Vec<Vec<TexRef>>,
    /// Per shader, every texture parameter: its name hash and texture, in
    /// the shader's order (colour, normal, specular, masks, palettes…).
    pub params: Vec<Vec<(u32, TexRef)>>,
    /// Per shader, its render bucket (see `Bucket`).
    pub buckets: Vec<Bucket>,
    /// Per shader, whether it blends terrain layers through a blend map
    /// (`lookuptexture`): its layers tile by the meshes' second UV set.
    pub layered: Vec<bool>,
    /// The embedded dictionary's textures, if any.
    pub embedded: Vec<txd::TextureInfo>,
    /// The skeleton's bones, if it has one (vehicles, characters).
    pub bones: Vec<Bone>,
    /// Further skeletons after the first (clothes keep their cloth bones,
    /// `GM_U01_…`, in several): their bones, each with its parent within
    /// its own skeleton dropped. Where these sit is all a viewer needs to
    /// let the body's bones carry them.
    pub more_bones: Vec<Bone>,
}

/// A bone: its name, where it sits in the model (bind pose; a 4×4 matrix
/// in rows, the translation in the last, so a point in the bone's space
/// times it is in the model's), and its parent.
#[derive(Clone, Debug, Default)]
pub struct Bone {
    pub name: String,
    pub matrix: [f32; 16],
    pub parent: Option<u16>,
}

/// The skeleton of the drawable at `b`: at its `+0x18`, the bone count
/// (u16) at `+0x6c` (`+0x68` holds a hash), bone records (80 bytes: the
/// name pointer at `+0x30`, next sibling and parent as u16 at `+0x44` and
/// `+0x46`) at `+0x28`, and per bone, at `+0x30`, the matrix that takes the
/// model's space into the bone's (rows; the inverse bind pose), whose
/// inverse places the bone. Measured on vehicles: the wheels sit at their
/// bones, the driver's seat and steering wheel on the left (−x), the front
/// at +y, as in the earlier games.
pub fn bones(v: &[u8], b: usize) -> Vec<Bone> {
    let Some(sk) = vptr(le::u64(v, b + 0x18)) else { return Vec::new() };
    skeleton_at(v, sk)
}

/// The skeletons that follow the first, 0x80 bytes apart, while they look
/// like skeletons (a clothing model's cloth bones), their bones together.
pub fn more_bones(v: &[u8], b: usize) -> Vec<Bone> {
    let Some(first) = vptr(le::u64(v, b + 0x18)) else { return Vec::new() };
    let mut out = Vec::new();
    for k in 1..32 {
        let sk = first + k * 0x80;
        if sk + 0x80 > v.len() || le::u16(v, sk + 0x6c) != le::u16(v, sk + 0x6e) || le::u64(v, sk) != 0 {
            break;
        }
        let bones = skeleton_at(v, sk);
        if bones.is_empty() || !bones[0].name.is_ascii() || bones[0].name.is_empty() {
            break;
        }
        out.extend(bones.into_iter().map(|b| Bone { parent: None, ..b }));
    }
    out
}

fn skeleton_at(v: &[u8], sk: usize) -> Vec<Bone> {
    let n = le::u16(v, sk + 0x6c) as usize;
    let (Some(recs), Some(mats)) = (vptr(le::u64(v, sk + 0x28)), vptr(le::u64(v, sk + 0x30))) else { return Vec::new() };
    if n == 0 || n > 4096 || recs + n * 80 > v.len() || mats + n * 64 > v.len() {
        return Vec::new();
    }
    (0..n)
        .map(|i| {
            let name = vptr(le::u64(v, recs + i * 80 + 0x30)).map(|p| String::from_utf8_lossy(le::cstr(v, p)).into_owned()).unwrap_or_default();
            let inv: [f32; 16] = std::array::from_fn(|k| f32::from_bits(le::u32(v, mats + i * 64 + k * 4)));
            let parent = Some(le::u16(v, recs + i * 80 + 0x46)).filter(|&p| (p as usize) < n);
            Bone { name, matrix: invert_affine(&inv), parent }
        })
        .collect()
}

/// Where a shader draws, by the low four bits of the byte at `+0x39` of
/// the shader struct (the bit `0x80` there is a separate flag, set on most
/// props). The same numbering as the earlier RAGE games' draw buckets;
/// measured on walls, glass, decal sets, foliage, fences and water.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Bucket {
    /// 0, and the buckets not told apart yet.
    #[default]
    Opaque,
    /// 1 (glass, windows) and 7: blended over what is behind.
    Alpha,
    /// 2: drawn over the surface under it (dirt, markings, stains).
    Decal,
    /// 3: cut out by the texture's alpha (leaves, fences).
    Cutout,
    /// 6: water surfaces.
    Water,
}

impl Bucket {
    fn of(byte: u8) -> Bucket {
        match byte & 0x0f {
            1 | 7 => Bucket::Alpha,
            2 => Bucket::Decal,
            3 => Bucket::Cutout,
            6 => Bucket::Water,
            _ => Bucket::Opaque,
        }
    }
}

fn f3(v: &[u8], o: usize) -> [f32; 3] {
    [0, 4, 8].map(|i| f32::from_bits(le::u32(v, o + i)))
}

/// Where the drawable starts: 0, or a fragment's drawable.
pub fn base(v: &[u8]) -> Option<usize> {
    if lod0(v, 0).is_some() {
        return Some(0);
    }
    let b = vptr(le::u64(v, 0x18))?;
    lod0(v, b).map(|_| b)
}

fn lod0(v: &[u8], b: usize) -> Option<(usize, usize)> {
    let l = vptr(le::u64(v, b + 0x50))?;
    let models = vptr(le::u64(v, l))?;
    let n = le::u16(v, l + 8) as usize;
    (n > 0 && n <= 256 && models + n * 8 <= v.len()).then_some((models, n))
}

pub fn name(v: &[u8], b: usize) -> String {
    vptr(le::u64(v, b + 0xa8)).map(|o| String::from_utf8_lossy(le::cstr(v, o)).into_owned()).unwrap_or_default()
}

pub fn parse(v: &[u8], p: &[u8]) -> Option<Drawable> {
    parse_at(v, p, base(v)?)
}

/// The model a file holds: its drawable, or, when the file is a drawable
/// dictionary (as most character, animal and prop models in the packs
/// are, even those named `.#dr`), the entry whose name hash is `hash`, else
/// the first.
pub fn parse_model(v: &[u8], p: &[u8], hash: u32) -> Option<Drawable> {
    if let Some(d) = parse(v, p) {
        return Some(d);
    }
    let n = le::u16(v, 0x38) as usize;
    if n == 0 || n != le::u16(v, 0x28) as usize {
        return None;
    }
    let hashes = vptr(le::u64(v, 0x20))?;
    let k = (0..n).find(|&k| le::u32(v, hashes + k * 4) == hash).unwrap_or(0);
    parse_at(v, p, dictionary_entry(v, k)?)
}

/// The drawable at entry `k` of a drawable dictionary (a pgDictionary:
/// `+0x20` name hashes, `+0x30` items, u16 counts at `+0x28`/`+0x38`).
pub fn dictionary_entry(v: &[u8], k: usize) -> Option<usize> {
    let items = vptr(le::u64(v, 0x30))?;
    if k >= le::u16(v, 0x38) as usize {
        return None;
    }
    vptr(le::u64(v, items + k * 8))
}

/// The drawable whose structure starts at `b`.
pub fn parse_at(v: &[u8], p: &[u8], b: usize) -> Option<Drawable> {
    let mut d = Drawable {
        name: name(v, b),
        sphere: [f32::from_bits(le::u32(v, b + 0x20)), f32::from_bits(le::u32(v, b + 0x24)), f32::from_bits(le::u32(v, b + 0x28)), f32::from_bits(le::u32(v, b + 0x2c))],
        min: f3(v, b + 0x30),
        max: f3(v, b + 0x40),
        ..Drawable::default()
    };
    d.bones = bones(v, b);
    d.more_bones = more_bones(v, b);
    // Shaders and their diffuse maps.
    if let Some(sg) = vptr(le::u64(v, b + 0x10)) {
        if let Some(td) = vptr(le::u64(v, sg + 8)) {
            d.embedded = embedded_at(v, td);
        }
        let shaders = vptr(le::u64(v, sg + 0x10));
        let n = le::u16(v, sg + 0x18) as usize;
        if let Some(sa) = shaders.filter(|_| n <= 1024) {
            for si in 0..n {
                let sh = vptr(le::u64(v, sa + si * 8));
                let params = sh.map(|sh| texture_params(v, sh)).unwrap_or_default();
                d.shaders.push(albedo(&params));
                d.params.push(params);
                d.buckets.push(sh.map(|sh| Bucket::of(le::u8(v, sh + 0x39))).unwrap_or_default());
                d.layered.push(sh.is_some_and(|sh| has_param(v, sh, LOOKUP)));
            }
        }
    }
    let (models, n) = lod0(v, b)?;
    for mi in 0..n {
        let Some(m) = vptr(le::u64(v, models + mi * 8)) else { continue };
        let Some(ga) = vptr(le::u64(v, m + 8)) else { continue };
        let gn = le::u16(v, m + 0x10) as usize;
        let map = vptr(le::u64(v, m + 0x20));
        let palette = palette(v, m);
        for gi in 0..gn.min(1024) {
            let Some(g) = vptr(le::u64(v, ga + gi * 8)) else { continue };
            if le::u64(v, g + 0x60) != 0x1601 {
                continue;
            }
            let shader = map.map_or(0, |s| le::u16(v, s + gi * 2) as usize);
            if let Some(mut mesh) = geometry(v, p, g) {
                mesh.shader = shader;
                if !mesh.joints.is_empty() {
                    mesh.palette = palette.clone();
                }
                d.meshes.push(mesh);
            }
        }
    }
    Some(d)
}

/// The bone palette of the model at `m`: the sorted name hashes its
/// vertices' bones index; empty when the model indexes the skeleton itself
/// (its `+0x28` then holds a bit set, not sorted hashes).
fn palette(v: &[u8], m: usize) -> Vec<u32> {
    let n = le::u16(v, m + 0x14) as usize;
    let Some(at) = vptr(le::u64(v, m + 0x28)) else { return Vec::new() };
    if n == 0 || at + n * 4 > v.len() {
        return Vec::new();
    }
    let hashes: Vec<u32> = (0..n).map(|k| le::u32(v, at + k * 4)).collect();
    if hashes.windows(2).all(|w| w[0] < w[1]) { hashes } else { Vec::new() }
}

/// The inverse of an affine matrix in rows (rotation in the first three
/// rows, translation in the last) whose rotation is orthonormal.
fn invert_affine(m: &[f32; 16]) -> [f32; 16] {
    let mut o = [0.0f32; 16];
    for r in 0..3 {
        for c in 0..3 {
            o[r * 4 + c] = m[c * 4 + r];
        }
    }
    for j in 0..3 {
        o[12 + j] = -(0..3).map(|i| m[12 + i] * m[j * 4 + i]).sum::<f32>();
    }
    o[15] = 1.0;
    o
}

/// Texture infos of the dictionary at `td`, with offsets kept absolute.
fn embedded_at(v: &[u8], td: usize) -> Vec<txd::TextureInfo> {
    let Some(items) = vptr(le::u64(v, td + 0x30)) else { return Vec::new() };
    let n = le::u16(v, td + 0x38) as usize;
    if n == 0 || n > 4096 || items + n * 8 > v.len() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for i in 0..n {
        let Some(t) = vptr(le::u64(v, items + i * 8)) else { continue };
        out.push(txd::TextureInfo {
            name: vptr(le::u64(v, t + 0x30)).map(|o| String::from_utf8_lossy(le::cstr(v, if le::u64(v, o) == 0 { o + 8 } else { o })).into_owned()).unwrap_or_default(),
            width: le::u16(v, t + 0x38) as u32,
            height: le::u16(v, t + 0x3a) as u32,
            dxgi: le::u8(v, t + 0x3f),
            tiled: le::u8(v, t + 0x40) != 0,
            mips: le::u8(v, t + 0x42),
            data: pptr(le::u64(v, t + 0x18)),
            at: t,
        });
    }
    out
}

/// `lookuptexture`: the terrain blend shader's blend map.
const LOOKUP: u32 = 0xc075_f9b6;

/// Whether the shader at `sh` has a parameter named by `hash`.
fn has_param(v: &[u8], sh: usize, hash: u32) -> bool {
    let Some(info) = vptr(le::u64(v, sh + 0x20)) else { return false };
    (0..le::u8(v, info + 4) as usize).any(|i| le::u32(v, info + 8 + i * 8) == hash)
}

/// Every texture parameter of the shader at `sh`: (name hash, texture).
pub fn texture_params(v: &[u8], sh: usize) -> Vec<(u32, TexRef)> {
    let (Some(refs), Some(info)) = (vptr(le::u64(v, sh + 0x10)), vptr(le::u64(v, sh + 0x20))) else { return Vec::new() };
    let (nt, np) = (le::u8(v, info + 1) as usize, le::u8(v, info + 4) as usize);
    let mut out = Vec::new();
    for i in 0..np {
        let (hash, data) = (le::u32(v, info + 8 + i * 8), le::u32(v, info + 12 + i * 8));
        let ti = ((data >> 2) & 255) as usize;
        if data & 3 != 0 || ti >= nt {
            continue;
        }
        let Some(tp) = vptr(le::u64(v, refs + ti * 8)) else { continue };
        // An embedded texture is referenced directly by its struct.
        let t = if le::u64(v, tp + 0x18) >> 28 == 6 {
            TexRef::Embedded(tp)
        } else {
            match vptr(le::u64(v, tp + 0x30)).map(|r| (le::u64(v, r), le::u64(v, r + 8))) {
                Some((ptr, key)) if vptr(ptr).is_some() && key != 0 => TexRef::Key(key),
                Some(_) => TexRef::Embedded(tp),
                None => continue,
            }
        };
        out.push((hash, t));
    }
    out
}

/// The albedo maps among a shader's texture parameters, best first.
fn albedo(params: &[(u32, TexRef)]) -> Vec<TexRef> {
    let mut found: Vec<(usize, &TexRef)> = params.iter().filter_map(|(h, t)| ALBEDO.iter().position(|a| a == h).map(|rank| (rank, t))).collect();
    found.sort_by_key(|f| f.0);
    found.into_iter().map(|f| f.1.clone()).collect()
}

/// Which model axis each packed position slot holds, by the vertex buffer's
/// `+0x6c`: the axis in the 16-bit slot (usually the mesh's thinnest), the
/// two 24-bit slots holding the others in this order. Checked against the
/// declared bounds of ~350 meshes, with no sign flips.
fn slot_axes(k: u32) -> [usize; 3] {
    match k {
        0 => [2, 1, 0],
        1 => [2, 0, 1],
        _ => [0, 1, 2],
    }
}

fn geometry(v: &[u8], p: &[u8], g: usize) -> Option<Mesh> {
    let vb = pptr(le::u64(v, g + 0x68))?;
    let vb_size = le::u32(v, g + 0x58) as usize * 4;
    let link = vptr(le::u64(v, g + 0x40))?;
    let mo = vptr(le::u64(v, link))?;
    let mb = pptr(le::u64(v, mo + 0x18))?;
    if mb + 12 > p.len() || vb + 0x80 > p.len() {
        return None;
    }
    let meshlets = (le::u32(p, mb) & 0xffff) as usize;
    let tris = le::u32(p, mb + 4) as usize;
    let refs = le::u32(p, mb + 8) as usize;
    let tri_base = mb + 12 + meshlets * 16;
    let remap = tri_base + tris * 4;
    if meshlets == 0 || remap + refs * 2 > p.len() {
        return None;
    }
    let heads: Vec<u32> = (0..8).map(|k| le::u32(p, vb + k * 4)).collect();
    let pos_off = ((heads[0] >> 8) << 1) as usize;
    let mut pos_end = vb_size;
    // UV0: a 0x28 stream (u16, 1/8192) or the first 0x42 stream after the
    // positions (two f32); UV1: the next such stream.
    let (mut uv, mut uv1): (Option<(usize, bool)>, Option<(usize, bool)>) = (None, None);
    for &h in &heads[1..] {
        if h == 0 {
            continue;
        }
        let o = ((h >> 8) << 1) as usize;
        if o > pos_off && o < pos_end {
            pos_end = o;
        }
        let this = match h & 0xff {
            0x28 => Some((o, false)),
            0x42 => Some((o, true)),
            _ => None,
        };
        match (uv, uv1) {
            (None, _) => uv = this,
            (Some(_), None) => uv1 = this,
            _ => {}
        }
    }
    // Positions are packed (type 0x42, 8 bytes, see `slot_axes`) or plain
    // floats (type 0x63, 12 bytes, as on cables and some decals).
    let float_pos = heads[0] & 0xff == 0x63;
    let stride = if float_pos { 12 } else { 8 };
    let nverts = ((pos_end.saturating_sub(pos_off)) / stride).min(p.len().saturating_sub(vb + pos_off) / stride);
    let off = f3(p, vb + 0x60);
    let sc = f3(p, vb + 0x70);
    let mut mesh = Mesh { streams: heads.iter().filter(|&&h| h != 0).map(|&h| ((h & 0xff) as u8, ((h >> 8) << 1) as usize)).collect(), vb, ..Mesh::default() };
    let mut used = 0usize;
    for mi in 0..meshlets {
        let m = mb + 12 + mi * 16;
        let vref = le::u16(p, m + 4) as usize;
        let t0 = le::u16(p, m + 6) as usize;
        let t1 = if mi + 1 < meshlets { le::u16(p, m + 16 + 6) as usize } else { tris };
        for t in t0..t1.min(tris) {
            let w = le::u32(p, tri_base + t * 4);
            let r = [w & 0x7f, (w >> 7) & 0x7f, (w >> 14) & 0x7f].map(|x| vref + x as usize);
            if r.iter().any(|&x| x >= refs) {
                continue;
            }
            let ix = r.map(|x| le::u16(p, remap + x * 2) as usize);
            if ix.iter().any(|&i| i >= nverts) || ix[0] == ix[1] || ix[1] == ix[2] || ix[0] == ix[2] {
                continue;
            }
            used = used.max(ix[0]).max(ix[1]).max(ix[2]);
            mesh.indices.extend(ix.map(|i| i as u32));
        }
    }
    if mesh.indices.is_empty() {
        return None;
    }
    let base = vb + pos_off;
    let axes = slot_axes(le::u32(p, vb + 0x6c));
    for i in 0..=used {
        let o = base + i * stride;
        if float_pos {
            mesh.positions.push(f3(p, o));
        } else {
            let s0 = u32::from(p[o + 1]) | u32::from(p[o + 2]) << 8 | u32::from(p[o + 3]) << 16;
            let s1 = u32::from(p[o]) | u32::from(p[o + 4]) << 8 | u32::from(p[o + 5]) << 16;
            let s2 = u32::from(p[o + 6]) | u32::from(p[o + 7]) << 8;
            let mut pos = [0.0; 3];
            for (j, q) in [s0, s1, s2].into_iter().enumerate() {
                pos[axes[j]] = off[j] + q as f32 * sc[j];
            }
            mesh.positions.push(pos);
        }
        let read = |s: Option<(usize, bool)>| match s {
            Some((u, true)) if vb + u + i * 8 + 8 <= p.len() => {
                let a = vb + u + i * 8;
                [f32::from_bits(le::u32(p, a)), f32::from_bits(le::u32(p, a + 4))]
            }
            Some((u, false)) if vb + u + i * 4 + 4 <= p.len() => {
                let a = vb + u + i * 4;
                [le::u16(p, a) as f32 / 8192.0, le::u16(p, a + 2) as f32 / 8192.0]
            }
            _ => [0.0, 0.0],
        };
        mesh.uvs.push(read(uv));
        if uv1.is_some() {
            mesh.uvs1.push(read(uv1));
        }
    }
    skin(p, vb, used + 1, &mut mesh);
    Some(mesh)
}

/// Bytes per vertex of a joints stream, by its type: one bone (`0x00`, a
/// vehicle's rigid piece), four as u8 (`0x25`), as u16 (`0x4a`), or as the
/// low halves of four u32 (`0x8c`, heads).
fn joint_size(t: u8) -> Option<usize> {
    match t {
        0x00 => Some(1),
        0x25 => Some(4),
        0x4a => Some(8),
        0x8c => Some(16),
        _ => None,
    }
}

fn read_joints(p: &[u8], a: usize, t: u8, out: &mut [u16]) {
    for (k, j) in out.iter_mut().enumerate().take(if t == 0 { 1 } else { 4 }) {
        *j = match t {
            0x00 | 0x25 => p[a + k] as u16,
            0x4a => le::u16(p, a + 2 * k),
            _ => le::u16(p, a + 4 * k),
        };
    }
}

/// The bones the vertices follow, from slots 6–9 (see the module notes).
fn skin(p: &[u8], vb: usize, n: usize, mesh: &mut Mesh) {
    let slot = |k: usize| {
        let h = le::u32(p, vb + k * 4);
        (h != 0).then_some(((h & 0xff) as u8, ((h >> 8) << 1) as usize))
    };
    let Some((jt, jo)) = slot(8) else { return };
    let Some(size) = joint_size(jt) else { return };
    let second = slot(9).filter(|s| s.0 == jt);
    let (w0, w1) = (slot(6).filter(|s| s.0 == 0x26), slot(7).filter(|s| s.0 == 0x26));
    let fits = |o: usize, size: usize| vb + o + n * size <= p.len();
    if !fits(jo, size) || second.is_some_and(|s| !fits(s.1, size)) || w0.is_some_and(|s| !fits(s.1, 4)) || w1.is_some_and(|s| !fits(s.1, 4)) {
        return;
    }
    mesh.joints.reserve(n);
    mesh.weights.reserve(n);
    for i in 0..n {
        let (mut j, mut w) = ([0u16; 8], [0u8; 8]);
        read_joints(p, vb + jo + i * size, jt, &mut j[..4]);
        if let Some(s) = second {
            read_joints(p, vb + s.1 + i * size, jt, &mut j[4..]);
        }
        match w0 {
            Some(s) => w[..4].copy_from_slice(&p[vb + s.1 + i * 4..vb + s.1 + i * 4 + 4]),
            None => w[0] = 255,
        }
        if let (Some(s), Some(_)) = (w1, second) {
            w[4..].copy_from_slice(&p[vb + s.1 + i * 4..vb + s.1 + i * 4 + 4]);
        }
        mesh.joints.push(j);
        mesh.weights.push(w);
    }
}

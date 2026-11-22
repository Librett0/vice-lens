//! Map data: placed entities, stored in `0DRPz` parsable resources.
//!
//! A `0DRPz` resource lists its blocks from the header: `+0x38` pointer to
//! 16-byte records (`u32 type, u32 size, u64 pointer`), `+0x4c` u16 count.
//! A pointer may land in the virtual or the physical segment.
//!
//! Entities are blocks of type `0xce501483`, 144 bytes each:
//! `+0x08` u64 id, `+0x10` rotation quaternion (x, y, z, w), `+0x38`
//! position (3 × f32, world), `+0x44` model name (joaat), `+0x48` flags,
//! `+0x54` scale XY, `+0x58` scale Z, `+0x5c` parent (packed; `0xffff0000`
//! on parentless LOD entities, -1 on parentless detailed ones), `+0x60`
//! LOD distance (-1: the archetype's), `+0x64` the distance within which
//! its children (the detailed models a LOD stands in for) are drawn,
//! `+0x68` the number of children (0 on detailed models, 1 on most LODs,
//! 4 or 16 on super-LODs).
//! Each entity's children hang under it by distance: detailed models
//! near, its LOD farther, the super-LODs farther still.
//!
//! Instance batches (`0xba049770`, 368 bytes) start like an entity (model
//! at `+0x44`); `+0x90` points to 64-byte instances (`+0x98` u16 count):
//! a row-major 3×4 world matrix, then scale and tint.
//!
//! Interiors (MLOs: shops, flats, offices) keep their entities in their own
//! coordinates around the origin, in the resource that defines them:
//! blocks `0x10506455`, 544 bytes each, `+0x08` the interior's 64-bit hash,
//! `+0x38` LOD distance, `+0x40`/`+0x4c` bounds, `+0xc0` a pointer to an
//! array of pointers to its own entities (`+0xc8` u16 count), `+0xd0` rooms,
//! `+0xe0` portals, `+0xf0` entity sets (see `mlos`). The world places
//! an interior with a block `0x09cc28be` (592 bytes) in another resource: an
//! entity (rotation `+0x10`, the interior's hash at `+0x20`, position
//! `+0x38`, a model hash naming no drawable at `+0x44`, scale `+0x54`/`+0x58`)
//! followed by fields of its own. One interior can be placed many times (a
//! room type at nine spots).

use crate::le;
use crate::rsc::{pptr, vptr};

pub const ENTITY: u32 = 0xce50_1483;
pub const ARCHETYPE: u32 = 0x82d6_fc83;
const ARCHETYPE_SIZE: usize = 184;

/// What Vice Lens reads of an archetype (a model's shared definition).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Archetype {
    /// joaat of the model name.
    pub name: u32,
    /// Distance within which the model is drawn.
    pub lod_dist: f32,
    /// Bounding sphere radius.
    pub radius: f32,
    /// Distance within which its full-size textures are wanted.
    pub hd_texture_dist: f32,
}

/// Archetypes: blocks of type `0x82d6fc83`, 184-byte records: `+0x30`
/// flags, `+0x38` LOD distance, `+0x40`/`+0x4c` bounding box min/max,
/// `+0x58` sphere centre, `+0x64` radius, `+0x70` full-texture distance,
/// `+0x74` model name (joaat; repeated at `+0x78`, `+0x80`, `+0x88` as its
/// dictionary names).
pub fn archetypes(virt: &[u8], phys: &[u8]) -> Vec<Archetype> {
    let mut out = Vec::new();
    for (t, b) in blocks(virt, phys) {
        if t != ARCHETYPE {
            continue;
        }
        for r in b.as_chunks::<ARCHETYPE_SIZE>().0 {
            let f = |o: usize| f32::from_bits(le::u32(r, o));
            out.push(Archetype { name: le::u32(r, 0x74), lod_dist: f(0x38), radius: f(0x64), hd_texture_dist: f(0x70) });
        }
    }
    out
}
pub const BATCH: u32 = 0xba04_9770;
const BATCH_SIZE: usize = 368;
pub const ENTITY_SIZE: usize = 144;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entity {
    pub id: u64,
    pub model: u32,
    pub pos: [f32; 3],
    pub rot: [f32; 4],
    pub scale_xy: f32,
    pub scale_z: f32,
    pub flags: u32,
    pub parent: i32,
    pub lod_dist: f32,
    pub child_lod_dist: f32,
    pub children: u32,
}

/// The blocks of a `0DRPz` resource: (type, size, bytes).
pub fn blocks<'a>(virt: &'a [u8], phys: &'a [u8]) -> Vec<(u32, &'a [u8])> {
    let Some(list) = vptr(le::u64(virt, 0x38)) else { return Vec::new() };
    let n = le::u16(virt, 0x4c) as usize;
    let mut out = Vec::new();
    for i in 0..n.min(4096) {
        let r = list + i * 16;
        let (t, size, p) = (le::u32(virt, r), le::u32(virt, r + 4) as usize, le::u64(virt, r + 8));
        let bytes = if let Some(o) = vptr(p) {
            virt.get(o..o + size)
        } else if let Some(o) = pptr(p) {
            phys.get(o..o + size)
        } else {
            None
        };
        if let Some(b) = bytes {
            out.push((t, b));
        }
    }
    out
}

pub fn entities(virt: &[u8], phys: &[u8]) -> Vec<Entity> {
    let mut out = Vec::new();
    for (t, b) in blocks(virt, phys) {
        if t != ENTITY {
            continue;
        }
        out.extend(b.as_chunks::<ENTITY_SIZE>().0.iter().map(|e| entity(e)));
    }
    out
}

/// An entity record (144 bytes; the start of an interior placement too).
fn entity(e: &[u8]) -> Entity {
    let f = |o: usize| f32::from_bits(le::u32(e, o));
    Entity {
        id: le::u64(e, 8),
        rot: [f(0x10), f(0x14), f(0x18), f(0x1c)],
        pos: [f(0x38), f(0x3c), f(0x40)],
        model: le::u32(e, 0x44),
        flags: le::u32(e, 0x48),
        scale_xy: f(0x54),
        scale_z: f(0x58),
        parent: le::u32(e, 0x5c) as i32,
        lod_dist: f(0x60),
        child_lod_dist: f(0x64),
        children: le::u32(e, 0x68),
    }
}

pub const MLO: u32 = 0x1050_6455;
const MLO_SIZE: usize = 544;
pub const MLO_INSTANCE: u32 = 0x09cc_28be;
const MLO_INSTANCE_SIZE: usize = 592;

/// An interior's definition: its hash and its entities, in its own
/// coordinates.
pub struct Mlo {
    pub hash: u64,
    pub entities: Vec<Entity>,
}

/// Where the world places an interior: its hash, and position, rotation
/// and scale as an entity's (the model naming no drawable).
pub struct MloInstance {
    pub hash: u64,
    pub place: Entity,
}

/// The interiors a resource defines, each with its own entities (the
/// shell, `+0xc0`) and its entity sets (`+0xf0`: 40-byte records, `+0x00`
/// name hash, `+0x18` entities, `+0x20` count) that scripts switch on: often
/// variants of one room's furnishing (39 for a house under construction,
/// the same decals in each). Which are on by default is not in the data;
/// sets are taken in order, and one is left out when over a third of its
/// entities stand where a set already taken put something: the first
/// variant stays, sets that add something else are kept.
pub fn mlos(virt: &[u8], phys: &[u8]) -> Vec<Mlo> {
    let blocks = blocks(virt, phys);
    let all = all_blocks(virt, phys);
    let entity_at = |p: u64| -> Option<Entity> {
        let (t, data) = all.get(((p & 0xfff) as usize).checked_sub(1)?)?;
        let rec = data.as_ref()?.get((p >> 20) as usize..)?.get(..ENTITY_SIZE)?;
        (*t == ENTITY).then(|| entity(rec))
    };
    // An array of pointers to entities: its pointer and count.
    let list = |p: u64, n: usize| -> Vec<Entity> {
        let Some(arr) = resolve(&blocks, &all, p) else { return Vec::new() };
        (0..n).filter_map(|k| arr.get(k * 8..k * 8 + 8)).filter_map(|w| entity_at(le::u64(w, 0))).collect()
    };
    let mut out = Vec::new();
    for (t, b) in &blocks {
        if *t != MLO {
            continue;
        }
        for d in b.as_chunks::<MLO_SIZE>().0 {
            let mut entities = list(le::u64(d, 0xc0), le::u16(d, 0xc8) as usize);
            let sets: Vec<Vec<Entity>> = match resolve(&blocks, &all, le::u64(d, 0xf0)) {
                Some(arr) => (0..le::u16(d, 0xf8) as usize).filter_map(|k| arr.get(k * 40..k * 40 + 40)).map(|s| list(le::u64(s, 0x18), le::u16(s, 0x20) as usize)).collect(),
                None => Vec::new(),
            };
            entities.extend(pick_sets(sets));
            out.push(Mlo { hash: le::u64(d, 8), entities });
        }
    }
    out
}

/// Entity sets in order, without those that are variants of one already
/// taken (over a third of their entities within 25 cm of its).
fn pick_sets(sets: Vec<Vec<Entity>>) -> Vec<Entity> {
    let mut taken: Vec<Entity> = Vec::new();
    for set in sets {
        let near = |e: &Entity| taken.iter().any(|t| (0..3).all(|c| (t.pos[c] - e.pos[c]).abs() < 0.25));
        let overlap = set.iter().filter(|e| near(e)).count();
        if overlap * 3 <= set.len() {
            taken.extend(set);
        }
    }
    taken
}

/// The interiors a resource places in the world.
pub fn mlo_instances(virt: &[u8], phys: &[u8]) -> Vec<MloInstance> {
    let mut out = Vec::new();
    for (t, b) in blocks(virt, phys) {
        if t != MLO_INSTANCE {
            continue;
        }
        out.extend(b.as_chunks::<MLO_INSTANCE_SIZE>().0.iter().map(|r| MloInstance { hash: le::u64(r, 0x20), place: entity(r) }));
    }
    out
}

/// An interior's entity placed in the world by one of its instances:
/// rotated and scaled by the instance, then moved to it.
pub fn place_in(inst: &Entity, e: &Entity) -> Entity {
    let q = inst.rot;
    let s = [inst.scale_xy, inst.scale_xy, inst.scale_z];
    let p = [e.pos[0] * s[0], e.pos[1] * s[1], e.pos[2] * s[2]];
    let r = rotate(q, p);
    Entity {
        pos: [inst.pos[0] + r[0], inst.pos[1] + r[1], inst.pos[2] + r[2]],
        rot: mul(q, e.rot),
        scale_xy: e.scale_xy * inst.scale_xy,
        scale_z: e.scale_z * inst.scale_z,
        parent: -1,
        child_lod_dist: 0.0,
        children: 0,
        ..*e
    }
}

/// Quaternion product `a b` (x, y, z, w).
fn mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let [ax, ay, az, aw] = a;
    let [bx, by, bz, bw] = b;
    [aw * bx + ax * bw + ay * bz - az * by, aw * by - ax * bz + ay * bw + az * bx, aw * bz + ax * by - ay * bx + az * bw, aw * bw - ax * bx - ay * by - az * bz]
}

/// `v` rotated by the unit quaternion `q`.
fn rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let r = mul(mul(q, [v[0], v[1], v[2], 0.0]), [-q[0], -q[1], -q[2], q[3]]);
    [r[0], r[1], r[2]]
}

/// Resolve a PSO-style pointer (block in bits 0..12, offset from bit 20)
/// against the resource's blocks.
fn resolve<'a>(blocks: &[(u32, &'a [u8])], all: &[(u32, Option<&'a [u8]>)], p: u64) -> Option<&'a [u8]> {
    let _ = blocks;
    let b = (p & 0xfff) as usize;
    if b == 0 || (p >> 12) & 0xff != 0 {
        return None;
    }
    let data = all.get(b - 1)?.1?;
    data.get((p >> 20) as usize..)
}

/// Every block in list order, including ones Vice Lens could not locate.
fn all_blocks<'a>(virt: &'a [u8], phys: &'a [u8]) -> Vec<(u32, Option<&'a [u8]>)> {
    let Some(list) = vptr(le::u64(virt, 0x38)) else { return Vec::new() };
    let n = le::u16(virt, 0x4c) as usize;
    (0..n.min(4096))
        .map(|i| {
            let r = list + i * 16;
            let (t, size, p) = (le::u32(virt, r), le::u32(virt, r + 4) as usize, le::u64(virt, r + 8));
            let bytes = vptr(p).and_then(|o| virt.get(o..o + size)).or_else(|| pptr(p).and_then(|o| phys.get(o..o + size)));
            (t, bytes)
        })
        .collect()
}

/// Instances of batched models: (model, row-major 3×4 world matrix).
pub fn instances(virt: &[u8], phys: &[u8]) -> Vec<(u32, [[f32; 4]; 3])> {
    let all = all_blocks(virt, phys);
    let blocks = blocks(virt, phys);
    let mut out = Vec::new();
    for (t, b) in &blocks {
        if *t != BATCH {
            continue;
        }
        for h in b.as_chunks::<BATCH_SIZE>().0 {
            let model = le::u32(h, 0x44);
            let n = le::u16(h, 0x98) as usize;
            let Some(arr) = resolve(&blocks, &all, le::u64(h, 0x90)) else { continue };
            for i in 0..n {
                let Some(r) = arr.get(i * 64..i * 64 + 48) else { break };
                let f = |o: usize| f32::from_bits(le::u32(r, o));
                out.push((model, [0, 16, 32].map(|row| [f(row), f(row + 4), f(row + 8), f(row + 12)])));
            }
        }
    }
    out
}

/// An instance matrix as an entity (rows: x axis | y axis | z axis with the
/// translation in the fourth column).
pub fn instance_entity(model: u32, m: [[f32; 4]; 3]) -> Option<Entity> {
    let col = |c: usize| [m[0][c], m[1][c], m[2][c]];
    let len = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let (x, y, z) = (col(0), col(1), col(2));
    let (sx, sy, sz) = (len(x), len(y), len(z));
    if !(sx > 0.0 && sy > 0.0 && sz > 0.0) || !m.iter().flatten().all(|v| v.is_finite()) {
        return None;
    }
    let n = |v: [f32; 3], s: f32| [v[0] / s, v[1] / s, v[2] / s];
    let (x, y, z) = (n(x, sx), n(y, sy), n(z, sz));
    // Rotation matrix (columns x, y, z) to quaternion.
    let tr = x[0] + y[1] + z[2];
    let q = if tr > 0.0 {
        let s = (tr + 1.0).sqrt() * 2.0;
        [(y[2] - z[1]) / s, (z[0] - x[2]) / s, (x[1] - y[0]) / s, 0.25 * s]
    } else if x[0] > y[1] && x[0] > z[2] {
        let s = (1.0 + x[0] - y[1] - z[2]).sqrt() * 2.0;
        [0.25 * s, (y[0] + x[1]) / s, (z[0] + x[2]) / s, (y[2] - z[1]) / s]
    } else if y[1] > z[2] {
        let s = (1.0 + y[1] - x[0] - z[2]).sqrt() * 2.0;
        [(y[0] + x[1]) / s, 0.25 * s, (z[1] + y[2]) / s, (z[0] - x[2]) / s]
    } else {
        let s = (1.0 + z[2] - x[0] - y[1]).sqrt() * 2.0;
        [(z[0] + x[2]) / s, (z[1] + y[2]) / s, 0.25 * s, (x[1] - y[0]) / s]
    };
    Some(Entity { id: 0, model, pos: [m[0][3], m[1][3], m[2][3]], rot: q, scale_xy: (sx + sy) / 2.0, scale_z: sz, flags: 0, parent: -2, lod_dist: -1.0, child_lod_dist: 0.0, children: 0 })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Of three furnishing variants of one room, the first is taken; a set
    /// that adds something elsewhere is kept.
    #[test]
    fn entity_set_variants_give_way_to_the_first() {
        let at = |model: u32, x: f32| Entity {
            id: 0,
            model,
            pos: [x, 0.0, 0.0],
            rot: [0.0, 0.0, 0.0, 1.0],
            scale_xy: 1.0,
            scale_z: 1.0,
            flags: 0,
            parent: -1,
            lod_dist: -1.0,
            child_lod_dist: 0.0,
            children: 0,
        };
        let sets = vec![vec![at(1, 0.0), at(2, 1.0)], vec![at(3, 0.0), at(4, 1.0)], vec![at(5, 9.0)], vec![at(6, 1.05), at(7, 20.0)]];
        let models: Vec<u32> = pick_sets(sets).iter().map(|e| e.model).collect();
        assert_eq!(models, [1, 2, 5]);
    }

    /// An interior turned a quarter left and moved: its entity a metre east
    /// of its origin lands a metre north of the placement, turned with it.
    #[test]
    fn interior_entities_turn_and_move_with_their_placement() {
        let (s, c) = (std::f32::consts::FRAC_PI_4.sin(), std::f32::consts::FRAC_PI_4.cos());
        let base = Entity { id: 0, model: 0, pos: [0.0; 3], rot: [0.0, 0.0, 0.0, 1.0], scale_xy: 1.0, scale_z: 1.0, flags: 0, parent: 7, lod_dist: -1.0, child_lod_dist: 5.0, children: 2 };
        let inst = Entity { pos: [10.0, 20.0, 3.0], rot: [0.0, 0.0, s, c], ..base };
        let e = Entity { pos: [1.0, 0.0, 0.5], ..base };
        let w = place_in(&inst, &e);
        for (got, want) in w.pos.iter().zip([10.0, 21.0, 3.5]) {
            assert!((got - want).abs() < 1e-5, "{:?}", w.pos);
        }
        for (got, want) in w.rot.iter().zip([0.0, 0.0, s, c]) {
            assert!((got - want).abs() < 1e-6, "{:?}", w.rot);
        }
        assert_eq!((w.parent, w.children), (-1, 0));
    }
}

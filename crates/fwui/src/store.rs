//! The game's style store, read from its two PSO files in data.rpf.
//!
//! - Styles (the large file): a map from style name hash to a list of
//!   states. A state (type `0x936422a6`) has an id at +28 (1 is the normal
//!   one; 3 and 4 hold larger text) and a list of properties: a text
//!   definition (type `0x817f7129`) and/or a brush (type `0x6909ec99`).
//! - A brush has three slots (+8, +24, +40), each a solid named colour
//!   reference (`hash`, `alpha %`) or a linear gradient (type `0xcb35f060`:
//!   stops of `f32 position, hash, alpha %`, an unidentified value at +0x20
//!   (0.45 or 0.25), 1.0 at +0x34, the angle in degrees at +0x48).
//! - Conversions (the small file): a map from conversion name hash to four
//!   style hashes: focused-disabled, focused-enabled, disabled, enabled.

use std::collections::HashMap;

use rage::pso::Pso;

const MAP_ENTRY: u32 = 0xebbc_845f;
const STATE: u32 = 0x9364_22a6;
const TEXT: u32 = 0x817f_7129;
const BRUSH: u32 = 0x6909_ec99;
const GRADIENT: u32 = 0xcb35_f060;
const CONVERSION_MAP: u32 = 0x100;
const CONVERSION: u32 = 0x9013_0782;

#[derive(Clone, Debug, PartialEq)]
pub enum Fill {
    /// Named colour hash and alpha 0..1.
    Solid(u32, f32),
    Gradient {
        stops: Vec<(f32, u32, f32)>,
        unknown: f32,
        angle: f32,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextDef {
    /// joaat of a `$fontN` alias.
    pub font: u32,
    pub size: f32,
    pub tracking: f32,
    pub flags: [u8; 8],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub id: u32,
    pub text: Option<TextDef>,
    pub brush: Option<[Option<Fill>; 3]>,
}

#[derive(Clone, Debug, Default)]
pub struct StyleStore {
    pub styles: HashMap<u32, Vec<State>>,
    pub conversions: HashMap<u32, [u32; 4]>,
}

impl StyleStore {
    pub fn load(styles: &Pso, conversions: &Pso) -> StyleStore {
        let mut s = StyleStore::default();
        if let Some(map) = styles.block_of_type(MAP_ENTRY) {
            for o in (map.offset..map.offset + map.size).step_by(24) {
                let hash = styles.u32(o);
                let arr = styles.u64(o + 8);
                let count = styles.u16(o + 16) as usize;
                let Some((at, _, _)) = styles.resolve(arr) else { continue };
                let states = (0..count).filter_map(|k| state(styles, styles.u64(at + 8 * k))).collect();
                s.styles.insert(hash, states);
            }
        }
        if let Some(map) = conversions.block_of_type(CONVERSION_MAP) {
            for o in (map.offset..map.offset + map.size).step_by(16) {
                let hash = conversions.u32(o);
                let Some((at, _, ty)) = conversions.resolve(conversions.u64(o + 8)) else { continue };
                if ty == CONVERSION {
                    let c = [0, 1, 2, 3].map(|i| conversions.u32(at + 16 + 4 * i));
                    s.conversions.insert(hash, c);
                }
            }
        }
        s
    }

    /// The state of a style for a text-size level (1 is normal), falling
    /// back to the first state.
    pub fn state(&self, style: u32, level: u32) -> Option<&State> {
        let states = self.styles.get(&style)?;
        states.iter().find(|s| s.id == level).or_else(|| states.first())
    }
}

fn state(p: &Pso, ptr: u64) -> Option<State> {
    let (at, _, ty) = p.resolve(ptr)?;
    if ty != STATE {
        return None;
    }
    let mut st = State { id: p.u32(at + 28), ..State::default() };
    let (arr, _, _) = p.resolve(p.u64(at + 8))?;
    for k in 0..p.u16(at + 16) as usize {
        let Some((o, _, t)) = p.resolve(p.u64(arr + 8 * k)) else { continue };
        match t {
            TEXT => {
                let mut flags = [0u8; 8];
                flags.copy_from_slice(&p.data[o + 0x20..o + 0x28]);
                st.text = Some(TextDef { font: p.u32(o + 8), size: p.u16(o + 0x1e) as f32, tracking: p.f32(o + 0x10), flags });
            }
            BRUSH => st.brush = Some([8, 24, 40].map(|slot| fill(p, p.u64(o + slot)))),
            _ => {}
        }
    }
    Some(st)
}

fn fill(p: &Pso, ptr: u64) -> Option<Fill> {
    let (o, block, ty) = p.resolve(ptr)?;
    match (block, ty) {
        // Block 2 holds the colour references.
        (2, _) => Some(Fill::Solid(p.u32(o + 8), p.u32(o + 12) as f32 / 100.0)),
        (_, GRADIENT) => {
            let (stops_at, _, _) = p.resolve(p.u64(o + 16))?;
            let n = p.u16(o + 24) as usize;
            let stops = (0..n)
                .map(|i| {
                    let s = stops_at + 12 * i;
                    (p.f32(s), p.u32(s + 4), p.u32(s + 8) as f32 / 100.0)
                })
                .collect();
            Some(Fill::Gradient { stops, unknown: p.f32(o + 0x20), angle: p.f32(o + 0x48) })
        }
        _ => None,
    }
}

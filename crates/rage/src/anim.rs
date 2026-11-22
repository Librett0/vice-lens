//! Clip dictionaries (`anim.rpf`): clips, the animations they play, and the
//! animations' channels, as `docs/formats.md` ("Animations") describes.
//!
//! The channel types the earlier games used (0–8) decode here: static
//! values, raw, quantised and indirectly quantised floats, linear floats,
//! and quaternions whose last component is computed; and type 10, the
//! incremental floats most animations now move with (see `incremental`).
//! Type 9 is not met; a block with it says so (`Block::unknown`).

use crate::le;
use crate::rsc::vptr;

/// A clip: its name, length in seconds, and the stretches of animations it
/// plays together.
#[derive(Clone, Debug)]
pub struct Clip {
    pub name: String,
    pub length: f32,
    pub parts: Vec<Part>,
}

/// A stretch of an animation: where it is (virtual offset) and the times
/// played, in the animation's seconds, at `rate`.
#[derive(Clone, Copy, Debug)]
pub struct Part {
    pub animation: usize,
    pub start: f32,
    pub end: f32,
    pub rate: f32,
}

/// One animated value of a bone: the bone's name hash (`joaat`, lower
/// case), the kind of track, and its format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Track {
    pub bone: u32,
    pub kind: u32,
    pub format: u32,
}

#[derive(Clone, Debug)]
pub struct Animation {
    pub name: String,
    pub frames: usize,
    /// Frames a block starts apart (each holds one more: blocks overlap by
    /// a frame).
    pub per_block: usize,
    pub length: f32,
    pub tracks: Vec<Track>,
    /// Blocks' virtual offsets.
    pub blocks: Vec<usize>,
}

fn cstr(v: &[u8], o: usize) -> String {
    String::from_utf8_lossy(le::cstr(v, o)).into_owned()
}

fn f32at(v: &[u8], o: usize) -> f32 {
    f32::from_bits(le::u32(v, o))
}

/// The dictionary's name and the structures its map holds (clips,
/// motions, others), or None when `v` is not one.
fn entries(v: &[u8]) -> Option<(String, Vec<usize>)> {
    let name = vptr(le::u64(v, 0x38)).filter(|&o| o < v.len())?;
    let map = vptr(le::u64(v, 0x48))?;
    let (buckets, count) = (le::u16(v, 0x40) as usize, le::u16(v, 0x42) as usize);
    if buckets == 0 || count == 0 || map + buckets * 8 > v.len() {
        return None;
    }
    let name = cstr(v, name);
    if name.len() < 3 || !name.is_ascii() {
        return None;
    }
    let mut out = Vec::new();
    for b in 0..buckets {
        let mut e = vptr(le::u64(v, map + b * 8));
        let mut guard = 0;
        while let Some(eo) = e.filter(|&eo| eo + 0x18 <= v.len()) {
            guard += 1;
            if guard > 4096 {
                break;
            }
            e = vptr(le::u64(v, eo + 0x10));
            if let Some(c) = vptr(le::u64(v, eo + 8)).filter(|&c| c + 0x80 <= v.len()) {
                out.push(c);
            }
        }
    }
    Some((name, out))
}

/// The dictionary's name and clips, or None when `v` is not one. A
/// cutscene's dictionary (`cin@…`) holds animations without clips
/// (`….anim`; a clip dictionary's map lists its packed animations too):
/// those are given as clips that play them whole.
pub fn dictionary(v: &[u8]) -> Option<(String, Vec<Clip>)> {
    let (name, at) = entries(v)?;
    let mut clips: Vec<Clip> = at.iter().filter_map(|&c| clip(v, c)).collect();
    if clips.is_empty() {
        clips = at.iter().filter_map(|&c| whole(v, c)).collect();
    }
    clips.sort_by(|a, b| a.name.cmp(&b.name));
    Some((name, clips))
}

/// A motion (`pack:/jog_run_stop.motion`): clips of other dictionaries
/// the game plays together, picking and syncing them by parameters each
/// clip carries over its time (how far is left to go; a phase that wraps
/// at 2π). The dictionaries without clips hold these.
#[derive(Clone, Debug)]
pub struct Motion {
    pub name: String,
    pub sources: Vec<Source>,
}

/// A clip a motion plays from: its name's hash (`clip_key`), its
/// dictionary's name hash, and per parameter its keys (value, time in s).
#[derive(Clone, Debug)]
pub struct Source {
    pub clip: u32,
    pub dictionary: u32,
    pub params: Vec<Vec<(f32, f32)>>,
}

/// The hash a motion names a clip by: of its name without `pack:/` and
/// `.clip` (`stop_left_rf_135`).
pub fn clip_key(name: &str) -> u32 {
    crate::joaat(name.trim_start_matches("pack:/").trim_end_matches(".clip"))
}

/// The motions of a dictionary. A motion: `+0x30` name (`….motion`),
/// `+0x60` its clips (pointer, u16 count; 16 bytes each, the clip's and
/// its dictionary's name hashes at `+8` and `+12`), `+0x80` per clip its
/// parameters (pointer, u16 count; 32 bytes each: keys pointer, u16 count,
/// then four floats), each key a value and a time.
pub fn motions(v: &[u8]) -> Vec<Motion> {
    let Some((_, at)) = entries(v) else { return Vec::new() };
    let mut out: Vec<Motion> = at.into_iter().filter_map(|m| motion(v, m)).collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn motion(v: &[u8], m: usize) -> Option<Motion> {
    if m + 0x90 > v.len() {
        return None;
    }
    let name = cstr(v, vptr(le::u64(v, m + 0x30)).filter(|&o| o < v.len())?);
    if !name.ends_with(".motion") {
        return None;
    }
    let list = |at: usize, size: usize| -> Option<(usize, usize)> {
        let (p, n) = (vptr(le::u64(v, m + at))?, le::u16(v, m + at + 8) as usize);
        (p + n * size <= v.len()).then_some((p, n))
    };
    let (refs, n) = list(0x60, 16)?;
    let params = list(0x80, 16).filter(|p| p.1 == n);
    let sources = (0..n)
        .map(|k| {
            let keys = |c: usize| -> Option<Vec<(f32, f32)>> {
                let (p, n) = (vptr(le::u64(v, c))?, le::u16(v, c + 8) as usize);
                (p + n * 8 <= v.len()).then(|| (0..n).map(|i| (f32at(v, p + i * 8), f32at(v, p + i * 8 + 4))).collect())
            };
            let params = params
                .and_then(|(p, _)| {
                    let (q, c) = (vptr(le::u64(v, p + k * 16))?, le::u16(v, p + k * 16 + 8) as usize);
                    (q + c * 32 <= v.len()).then(|| (0..c).map(|j| keys(q + j * 32).unwrap_or_default()).collect())
                })
                .unwrap_or_default();
            Source { clip: le::u32(v, refs + k * 16 + 8), dictionary: le::u32(v, refs + k * 16 + 12), params }
        })
        .collect();
    Some(Motion { name, sources })
}

/// An animation a dictionary holds itself, as a clip that plays it whole.
fn whole(v: &[u8], a: usize) -> Option<Clip> {
    let an = animation(v, a)?;
    (an.name.ends_with(".anim") && an.frames > 0 && an.length > 0.0).then(|| Clip {
        name: an.name.clone(),
        length: an.length,
        parts: vec![Part { animation: a, start: 0.0, end: an.length, rate: 1.0 }],
    })
}

/// The clip at `c`: of type 2 (a list of animations), the only kind met;
/// a dictionary's map also holds entries (key 0) that are not clips.
fn clip(v: &[u8], c: usize) -> Option<Clip> {
    if le::u32(v, c + 0x20) != 2 {
        return None;
    }
    let name = vptr(le::u64(v, c + 0x28)).map(|o| cstr(v, o)).unwrap_or_default();
    let lists = vptr(le::u64(v, c + 0x68)).filter(|&l| l + 0x18 <= v.len())?;
    let entries = vptr(le::u64(v, lists))?;
    let n = (le::u16(v, lists + 8) as usize).min(16);
    let parts = (0..n)
        .filter_map(|k| {
            let en = entries + k * 0x38;
            let animation = vptr(le::u64(v, en)).filter(|&a| a + 0x80 <= v.len())?;
            Some(Part { animation, start: f32at(v, en + 0x28), end: f32at(v, en + 0x2c), rate: f32at(v, en + 0x30) })
        })
        .collect();
    Some(Clip { name, length: f32at(v, c + 0x78), parts })
}

/// The animation at virtual offset `a`.
pub fn animation(v: &[u8], a: usize) -> Option<Animation> {
    if a + 0x80 > v.len() {
        return None;
    }
    let nt = le::u16(v, a + 0x68) as usize;
    let t = vptr(le::u64(v, a + 0x60))?;
    if t + nt * 12 > v.len() {
        return None;
    }
    let tracks = (0..nt).map(|i| Track { bone: le::u32(v, t + i * 12), kind: le::u32(v, t + i * 12 + 4), format: le::u32(v, t + i * 12 + 8) }).collect();
    let list = vptr(le::u64(v, a + 0x50))?;
    let blocks = (0..le::u16(v, a + 0x58) as usize).filter_map(|k| vptr(le::u64(v, list + k * 8)).filter(|&b| b + 0x20 <= v.len())).collect();
    Some(Animation {
        name: vptr(le::u64(v, a + 0x30)).map(|o| cstr(v, o)).unwrap_or_default(),
        frames: le::u16(v, a + 0x24) as usize,
        per_block: (le::u16(v, a + 0x26) as usize).max(1),
        length: f32at(v, a + 0x28),
        tracks,
        blocks,
    })
}

/// Bits from `bit` on, the first in the lowest bit of the first byte.
fn bits(d: &[u8], bit: usize, n: usize) -> u32 {
    let mut r = 0u32;
    for k in 0..n.min(32) {
        let b = bit + k;
        if d.get(b >> 3).is_some_and(|x| (x >> (b & 7)) & 1 == 1) {
            r |= 1 << k;
        }
    }
    r
}

#[derive(Clone, Debug)]
enum Chan {
    /// Values that do not change: a quaternion (4), a vector (3), a float (1).
    Fixed([f32; 4], usize),
    /// A value per frame.
    Frames(Vec<f32>),
    /// The quaternion component (0–3) computed from the other three.
    Computed(usize),
}

/// A decoded block: per track, its channels by component (0–4).
#[derive(Clone, Debug)]
pub struct Block {
    pub frames: usize,
    tracks: Vec<[Option<Chan>; 5]>,
    /// Channels of type 9 (not met yet): the block reads no further.
    pub unknown: usize,
}

/// Decode the block at virtual offset `b` of an animation with `tracks`.
pub fn block(v: &[u8], b: usize, tracks: usize) -> Option<Block> {
    let len = le::u32(v, b + 4) as usize;
    let frame_off = le::u32(v, b + 12) as usize;
    let frames = le::u16(v, b + 0x16) as usize;
    let flen = le::u16(v, b + 0x18) as usize;
    // The length counts the data after the 0x20-byte header.
    let d = v.get(b + 0x20..(b + 0x20 + len).min(v.len()))?;
    let list = frame_off + flen * frames;
    if list + 22 > d.len() {
        return None;
    }
    let counts: Vec<usize> = (0..11).map(|t| le::u16(d, list + t * 2) as usize).collect();
    let mut item = list + 22;
    let mut pos = 0usize;
    let mut frame_bit = 0usize;
    // Channels read per frame: (track, component, kind, bit offset).
    enum Per {
        Raw,
        Quant(usize, f32, f32),
        Indirect(usize, Vec<f32>),
    }
    let mut per: Vec<(usize, usize, Per, usize)> = Vec::new();
    let mut out = Block { frames, tracks: vec![Default::default(); tracks], unknown: counts[9] };
    for (t, &count) in counts.iter().enumerate() {
        // Type 9 is not met: its channels' data cannot be stepped over.
        if t == 9 && count > 0 {
            return Some(out);
        }
        for _ in 0..count {
            let it = le::u16(d, item) as usize;
            item += 2;
            let (seq, mut idx) = (it >> 2, it & 3);
            let chan = match t {
                0 | 1 => {
                    let x = [f32at(d, pos), f32at(d, pos + 4), f32at(d, pos + 8)];
                    pos += 12;
                    let w = if t == 0 { (1.0 - x[0] * x[0] - x[1] * x[1] - x[2] * x[2]).max(0.0).sqrt() } else { 0.0 };
                    Some(Chan::Fixed([x[0], x[1], x[2], w], if t == 0 { 4 } else { 3 }))
                }
                2 => {
                    let x = f32at(d, pos);
                    pos += 4;
                    Some(Chan::Fixed([x, 0.0, 0.0, 0.0], 1))
                }
                3 => {
                    per.push((seq, idx, Per::Raw, frame_bit));
                    frame_bit += 32;
                    None
                }
                4 => {
                    let (vb, q, o) = (le::u32(d, pos) as usize, f32at(d, pos + 4), f32at(d, pos + 8));
                    pos += 12;
                    per.push((seq, idx, Per::Quant(vb, q, o), frame_bit));
                    frame_bit += vb;
                    None
                }
                5 => {
                    let (fb, vb, ni) = (le::u32(d, pos) as usize, le::u32(d, pos + 4) as usize, le::u32(d, pos + 8) as usize);
                    let (q, o) = (f32at(d, pos + 12), f32at(d, pos + 16));
                    pos += 20;
                    let n = ((ni * 32) / vb.max(1)).min((1usize << fb.min(24)) - 1);
                    let values = (0..n).map(|i| bits(d, pos * 8 + i * vb, vb) as f32 * q + o).collect();
                    pos += ni * 4;
                    per.push((seq, idx, Per::Indirect(fb, values), frame_bit));
                    frame_bit += fb;
                    None
                }
                6 => {
                    let ni = le::u32(d, pos) as usize;
                    let values = linear(d, pos, frames, 64);
                    pos += ni * 4;
                    Some(Chan::Frames(values))
                }
                10 => {
                    let ni = le::u32(d, pos) as usize;
                    let values = incremental(d, pos, frames);
                    pos += ni * 4;
                    Some(Chan::Frames(values))
                }
                _ => {
                    let qi = idx;
                    idx = if t == 7 { 3 } else { 4 };
                    Some(Chan::Computed(qi))
                }
            };
            if let (Some(c), Some(slot)) = (chan, out.tracks.get_mut(seq)) {
                slot[idx.min(4)] = Some(c);
            }
        }
        if count % 4 != 0 {
            item += (4 - count % 4) * 2;
        }
    }
    // Values read frame by frame from the packed frame data.
    for (seq, idx, p, at) in per {
        let values = (0..frames)
            .map(|f| {
                let bit = (frame_off + flen * f) * 8 + at;
                match &p {
                    Per::Raw => f32::from_bits(bits(d, bit, 32)),
                    Per::Quant(vb, q, o) => bits(d, bit, *vb) as f32 * q + o,
                    Per::Indirect(fb, values) => values.get(bits(d, bit, *fb) as usize).copied().unwrap_or(0.0),
                }
            })
            .collect();
        if let Some(slot) = out.tracks.get_mut(seq) {
            slot[idx.min(4)] = Some(Chan::Frames(values));
        }
    }
    Some(out)
}

/// Incremental floats (type 10): after a header (u32 words with it, u32
/// bit widths c1 | c2 << 8 | c3 << 16, f32 quantum, f32 offset), per chunk
/// of 64 frames an offset (c1 bits) and a start value (c2 bits); then per
/// group of 8 frames of each chunk a width in unary (that many 0 bits, then
/// a 1); then, from the next 32-bit boundary, eight lanes interleaved word by
/// word (word w of lane l is word 8w + l). Frame 8g + 1 + l of a chunk is in
/// lane l with its group's width, a chunk starting at its offset in every
/// lane: a zigzag-coded second difference, the velocity starting at 0 in
/// each chunk. RDR2's type 9 is the same with groups and lanes of four.
fn incremental(d: &[u8], pos: usize, frames: usize) -> Vec<f32> {
    let (ni, cnt) = (le::u32(d, pos) as usize, le::u32(d, pos + 4));
    let (q, o) = (f32at(d, pos + 8), f32at(d, pos + 12));
    let (c1, c2) = ((cnt & 0xff) as usize, ((cnt >> 8) & 0xff) as usize);
    let chunks = frames.div_ceil(64);
    let bit = (pos + 16) * 8;
    let end = ((pos + ni * 4) * 8).min(d.len() * 8);
    let mut bp = bit + chunks * (c1 + c2);
    let mut widths: Vec<Vec<usize>> = Vec::with_capacity(chunks);
    for c in 0..chunks {
        let n = (frames - c * 64).min(64);
        let mut ws = Vec::with_capacity(8);
        for _ in 0..(n.saturating_sub(1)).div_ceil(8) {
            let mut z = 0;
            while bp < end && bits(d, bp, 1) == 0 {
                z += 1;
                bp += 1;
            }
            bp += 1;
            ws.push(z.min(32));
        }
        widths.push(ws);
    }
    let lanes = bit + (bp - bit).div_ceil(32) * 32;
    let lane = |l: usize, p: usize, w: usize| -> u32 {
        let mut r = 0u32;
        for k in 0..w {
            let at = p + k;
            if bits(d, lanes + (8 * (at / 32) + l) * 32 + at % 32, 1) == 1 {
                r |= 1 << k;
            }
        }
        r
    };
    let mut out = Vec::with_capacity(frames);
    for (c, ws) in widths.iter().enumerate() {
        let n = (frames - c * 64).min(64);
        let mut at = [bits(d, bit + c * c1, c1) as usize; 8];
        let mut value = bits(d, bit + chunks * c1 + c * c2, c2) as i64;
        let mut velocity = 0i64;
        out.push(value as f32 * q + o);
        for f in 1..n {
            let (l, w) = ((f - 1) % 8, ws.get((f - 1) / 8).copied().unwrap_or(0));
            let raw = lane(l, at[l], w) as i64;
            at[l] += w;
            velocity += if raw & 1 == 1 { -(raw + 1) / 2 } else { raw / 2 };
            value += velocity;
            out.push(value as f32 * q + o);
        }
    }
    out
}

/// The earlier games' linear floats: per chunk of frames a start value and
/// deltas of deltas (a few low bits, then the rest in unary, then a sign).
fn linear(d: &[u8], pos: usize, frames: usize, chunk: usize) -> Vec<f32> {
    let cnt = le::u32(d, pos + 4);
    let (q, o) = (f32at(d, pos + 8), f32at(d, pos + 12));
    let (c1, c2, c3) = ((cnt & 0xff) as usize, ((cnt >> 8) & 0xff) as usize, ((cnt >> 16) & 0xff) as usize);
    let bit = (pos + 16) * 8;
    let chunks = frames.div_ceil(chunk);
    let deltas = bit + chunks * (c1 + c2);
    let end = d.len() * 8;
    let mut out = vec![o; frames];
    for i in 0..chunks {
        let mut bp = deltas + bits(d, bit + i * c1, c1) as usize;
        let mut value = bits(d, bit + chunks * c1 + i * c2, c2) as i64;
        let mut inc = 0i64;
        for j in 0..chunk {
            let f = i * chunk + j;
            if f >= frames {
                break;
            }
            out[f] = value as f32 * q + o;
            if j + 1 >= chunk {
                break;
            }
            let mut delta = bits(d, bp, c3) as i64;
            bp += c3;
            let so = bp;
            while bp < end && bits(d, bp, 1) == 0 {
                bp += 1;
            }
            bp += 1;
            delta |= ((bp - so - 1) as i64) << c3;
            if delta != 0 {
                if bits(d, bp, 1) == 1 {
                    delta = -delta;
                }
                bp += 1;
            }
            inc += delta;
            value += inc;
        }
    }
    out
}

/// What a track holds at a frame: a rotation (x, y, z, w), or up to four
/// values (a position, a float); None when the block does not animate it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Rotation([f32; 4]),
    Values([f32; 4], usize),
}

impl Block {
    pub fn value(&self, track: usize, frame: usize) -> Option<Value> {
        let chans = self.tracks.get(track)?;
        let f = frame.min(self.frames.saturating_sub(1));
        let float = |c: &Chan| match c {
            Chan::Fixed(x, _) => x[0],
            Chan::Frames(v) => v.get(f).copied().unwrap_or(0.0),
            Chan::Computed(_) => 0.0,
        };
        // A quaternion with one component computed from the other three.
        if let Some(Chan::Computed(qi)) = chans[3].as_ref().or(chans[4].as_ref()) {
            let xyz = [0, 1, 2].map(|k| chans[k].as_ref().map_or(0.0, float));
            let w = (1.0 - xyz.iter().map(|x| x * x).sum::<f32>()).max(0.0).sqrt();
            let mut q = [0.0; 4];
            let mut k = 0;
            for (c, slot) in q.iter_mut().enumerate() {
                if c == *qi {
                    *slot = w;
                } else {
                    *slot = xyz[k];
                    k += 1;
                }
            }
            return Some(Value::Rotation(normalise(q)));
        }
        let mut v = [0.0f32; 4];
        let mut n = 0;
        let mut rotation = false;
        for c in chans.iter().take(4).flatten() {
            if n >= 4 {
                break;
            }
            match c {
                Chan::Fixed(x, wide) if *wide > 1 => {
                    // A static vector (3) or quaternion (4) fills several.
                    let wide = *wide;
                    rotation |= wide == 4;
                    for (k, val) in x.iter().take(wide).enumerate() {
                        if n + k < 4 {
                            v[n + k] = *val;
                        }
                    }
                    n += wide;
                }
                _ => {
                    v[n] = float(c);
                    n += 1;
                }
            }
        }
        if n == 0 {
            return None;
        }
        Some(if rotation || n == 4 { Value::Rotation(normalise(v)) } else { Value::Values(v, n.min(4)) })
    }
}

fn normalise(q: [f32; 4]) -> [f32; 4] {
    let l = q.iter().map(|x| x * x).sum::<f32>().sqrt();
    if l > 1e-6 { q.map(|x| x / l) } else { [0.0, 0.0, 0.0, 1.0] }
}

/// Every clip dictionary of an archive (`anim.rpf`, a content pack's):
/// offset, name, clips, and whether every clip decodes whole. One pass
/// over the archive's streams: a dictionary's header lies in its first, so
/// only those that look like one are read whole.
pub fn survey(a: &crate::Archive, progress: &dyn Fn(u64, u64)) -> Vec<(u64, String, usize, bool)> {
    let mut found = Vec::new();
    let mut streams: Vec<crate::Stream> = Vec::new();
    // A stream continues a file when it follows a full piece back to back
    // (as `Archive::files` groups them).
    let mut full_until: Option<u64> = None;
    a.scan(|s, d| {
        let starts = full_until != Some(s.offset);
        full_until = (s.size == crate::archive::PIECE).then(|| (s.offset + s.packed as u64 + 15) & !15);
        if streams.len().is_multiple_of(4096) {
            progress(s.offset, a.len());
        }
        let header = d.len() >= 0x60 && crate::rsc::looks_like_resource(d) && vptr(le::u64(d, 0x38)).is_some() && vptr(le::u64(d, 0x48)).is_some() && le::u16(d, 0x40) > 0 && le::u16(d, 0x42) > 0;
        if starts && header {
            found.push(streams.len());
        }
        streams.push(s);
        true
    });
    // Each read with the streams already found, not by scanning again.
    let next = |c: u64| {
        let at = (c + 15) & !15;
        let k = streams.partition_point(|s| s.offset < at);
        streams.get(k).copied()
    };
    let mut out = Vec::new();
    for k in found {
        let o = streams[k].offset;
        let Ok(r) = crate::rsc::read(a, streams[k], next) else { continue };
        let Some((name, clips)) = dictionary(&r.virt) else { continue };
        out.push((o, name, clips.len(), !clips.is_empty() && clips.iter().all(|c| readable(&r.virt, c))));
    }
    out
}

/// Whether every animation a clip plays decodes whole (no channel of type
/// 9, not met yet).
pub fn readable(v: &[u8], c: &Clip) -> bool {
    c.parts.iter().all(|p| {
        animation(v, p.animation).is_some_and(|an| {
            an.blocks.iter().all(|&b| {
                let len = le::u32(v, b + 4) as usize;
                let (fo, nf, fl) = (le::u32(v, b + 12) as usize, le::u16(v, b + 0x16) as usize, le::u16(v, b + 0x18) as usize);
                let at = b + 0x20 + fo + fl * nf;
                fo <= len && at + 22 <= v.len() && le::u16(v, at + 18) == 0
            })
        })
    })
}

/// A bone's local place at one moment: rotation (x, y, z, w) and position,
/// each when the clip animates it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pose {
    pub rotation: Option<[f32; 4]>,
    pub position: Option<[f32; 3]>,
}

/// Bone rotations and positions: the kinds of track that move bones.
pub const ROTATION: u32 = 0x98e1_750c;
pub const POSITION: u32 = 0xf791_a95f;

/// A clip sampled `fps` times a second over its length: per bone name hash,
/// its pose at each sample (the clip's later animations win where two move
/// one bone).
pub fn sample(v: &[u8], c: &Clip, fps: f32) -> (usize, std::collections::HashMap<u32, Vec<Pose>>) {
    let n = ((c.length * fps).round() as usize).max(1) + 1;
    let mut out: std::collections::HashMap<u32, Vec<Pose>> = Default::default();
    for p in &c.parts {
        let Some(an) = animation(v, p.animation) else { continue };
        let blocks: Vec<Option<Block>> = an.blocks.iter().map(|&b| block(v, b, an.tracks.len())).collect();
        let afps = if an.length > 0.0 { (an.frames.saturating_sub(1)) as f32 / an.length } else { 30.0 };
        for (ti, t) in an.tracks.iter().enumerate() {
            if t.kind != ROTATION && t.kind != POSITION {
                continue;
            }
            let at = |frame: usize| {
                let k = (frame / an.per_block).min(blocks.len().saturating_sub(1));
                blocks.get(k)?.as_ref()?.value(ti, frame - k * an.per_block)
            };
            for s in 0..n {
                let time = p.start + (s as f32 / fps) * p.rate;
                let f = (time * afps).clamp(0.0, an.frames.saturating_sub(1) as f32);
                let (f0, w) = (f.floor() as usize, f.fract());
                let f1 = (f0 + 1).min(an.frames.saturating_sub(1));
                let (Some(a0), Some(a1)) = (at(f0), at(f1)) else { continue };
                let pose = out.entry(t.bone).or_insert_with(|| vec![Pose::default(); n]);
                match (a0, a1) {
                    (Value::Rotation(q0), Value::Rotation(q1)) if t.kind == ROTATION => {
                        // The shorter way round, then normalised.
                        let dot: f32 = q0.iter().zip(&q1).map(|(a, b)| a * b).sum();
                        let s1 = if dot < 0.0 { -w } else { w };
                        let q = [0, 1, 2, 3].map(|k| q0[k] * (1.0 - w) + q1[k] * s1);
                        pose[s].rotation = Some(normalise(q));
                    }
                    (Value::Values(x0, k0), Value::Values(x1, _)) if t.kind == POSITION && k0 >= 3 => {
                        pose[s].position = Some([0, 1, 2].map(|k| x0[k] * (1.0 - w) + x1[k] * w));
                    }
                    _ => {}
                }
            }
        }
    }
    (n, out)
}

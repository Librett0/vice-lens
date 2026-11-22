//! Vice Lens: the sound banks in `audio.rpf`. Banks are AWC containers
//! (`ADAT`) stored as they are on 4 KiB boundaries; their single-channel
//! streams carry Opus packets in 2 KiB blocks, which Lens wraps, packet for
//! packet, into Ogg Opus for the page's audio player. Multi-channel music
//! (512 KiB blocks with a header of unknown layout) is listed but not
//! played yet.

use std::sync::Arc;

use super::Lens;
use super::http::{Reply, Request};

/// A bank: its offset in `audio.rpf`, its size, and how many streams.
pub(super) struct Bank {
    offset: u64,
    bytes: u32,
    streams: u32,
    music: bool,
}

/// One stream of a bank: its id (the low 29 bits of a name hash), and its
/// chunks (type, offset in the bank, size).
struct Stream {
    id: u32,
    chunks: Vec<(u8, u32, u32)>,
}

struct Awc {
    flags: u32,
    streams: Vec<Stream>,
}

const DATA: u8 = 0x55;
const FORMAT: [u8; 2] = [0x76, 0xfa];

fn u16le(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

/// Read an AWC header: flags, the optional tables the flags announce, the
/// stream table and the chunk table, which must end at the header length.
fn parse(b: &[u8]) -> Option<Awc> {
    if b.len() < 16 || &b[..4] != b"ADAT" {
        return None;
    }
    let (flags, n, header) = (u32le(b, 4), u32le(b, 8) as usize, u32le(b, 12) as usize);
    if n == 0 || n > 100_000 || header > b.len() || header < 16 {
        return None;
    }
    let mut p = 16;
    if flags & 0x0040_0000 != 0 {
        p += 4;
    }
    if flags & 0x0001_0000 != 0 {
        p += 2 * n;
    }
    if flags & 0x0020_0000 != 0 {
        p += 4 * (n + 1);
    }
    // The u16 table may leave the rest unaligned; try with and without a
    // pad to the next 4 bytes, keeping the one that ends at the header.
    for pad in [0, (4 - p % 4) % 4] {
        let q = p + pad;
        if q + 4 * n > header {
            continue;
        }
        let ids: Vec<(u32, usize)> = (0..n).map(|i| u32le(b, q + 4 * i)).map(|v| (v & 0x1fff_ffff, (v >> 29) as usize)).collect();
        let total: usize = ids.iter().map(|x| x.1).sum();
        if q + 4 * n + 8 * total != header {
            continue;
        }
        let mut at = q + 4 * n;
        let mut streams = Vec::with_capacity(n);
        for (id, count) in ids {
            let mut chunks = Vec::with_capacity(count);
            for _ in 0..count {
                let r = u64::from_le_bytes(b[at..at + 8].try_into().unwrap());
                at += 8;
                chunks.push(((r >> 56) as u8, (r & 0x0fff_ffff) as u32, ((r >> 28) & 0x0fff_ffff) as u32));
            }
            streams.push(Stream { id, chunks });
        }
        return Some(Awc { flags, streams });
    }
    None
}

/// The bank's extent: the end of its last chunk.
fn extent(a: &Awc) -> u32 {
    a.streams.iter().flat_map(|s| &s.chunks).map(|c| c.1 + c.2).max().unwrap_or(0)
}

/// Opus packets of a single-channel data chunk: 2 KiB blocks, each
/// `u16 length | packet` repeated, then zero padding.
fn packets(data: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    for block in data.chunks(0x800) {
        let mut p = 0;
        while p + 2 <= block.len() {
            let len = u16le(block, p) as usize;
            if len == 0 || len > 1500 || p + 2 + len > block.len() {
                break;
            }
            out.push(&block[p + 2..p + 2 + len]);
            p += 2 + len;
        }
    }
    out
}

/// Samples (at 48 kHz) an Opus packet holds, from its TOC byte.
fn opus_samples(pkt: &[u8]) -> u64 {
    let Some(&toc) = pkt.first() else { return 0 };
    let config = toc >> 3;
    let per_frame: u64 = match config {
        0..=11 => [480, 960, 1920, 2880][(config % 4) as usize],
        12..=15 => [480, 960][(config % 2) as usize],
        _ => [120, 240, 480, 960][(config % 4) as usize],
    };
    let frames = match toc & 3 {
        0 => 1,
        1 | 2 => 2,
        _ => pkt.get(1).map_or(1, |b| (b & 0x3f) as u64),
    };
    per_frame * frames
}

/// Ogg's CRC-32 (polynomial 0x04C11DB7, no reflection, no final XOR).
fn ogg_crc(data: &[u8]) -> u32 {
    let mut crc = 0u32;
    for &b in data {
        crc ^= (b as u32) << 24;
        for _ in 0..8 {
            crc = if crc & 0x8000_0000 != 0 { (crc << 1) ^ 0x04c1_1db7 } else { crc << 1 };
        }
    }
    crc
}

/// Ogg Opus around packets, copied as they are.
fn ogg_opus(pkts: &[&[u8]], channels: u8, rate: u32, pre_skip: u16) -> Vec<u8> {
    let mut out = Vec::new();
    let mut seq = 0u32;
    let serial: u32 = 0x4c45_4e53;
    let mut page = |out: &mut Vec<u8>, flags: u8, granule: u64, packets: &[&[u8]]| {
        let mut lacing = Vec::new();
        for p in packets {
            let mut n = p.len();
            while n >= 255 {
                lacing.push(255u8);
                n -= 255;
            }
            lacing.push(n as u8);
        }
        let start = out.len();
        out.extend_from_slice(b"OggS");
        out.push(0);
        out.push(flags);
        out.extend_from_slice(&granule.to_le_bytes());
        out.extend_from_slice(&serial.to_le_bytes());
        out.extend_from_slice(&seq.to_le_bytes());
        out.extend_from_slice(&[0; 4]);
        out.push(lacing.len() as u8);
        out.extend_from_slice(&lacing);
        for p in packets {
            out.extend_from_slice(p);
        }
        let crc = ogg_crc(&out[start..]);
        out[start + 22..start + 26].copy_from_slice(&crc.to_le_bytes());
        seq += 1;
    };
    let mut head = b"OpusHead".to_vec();
    head.push(1);
    head.push(channels);
    head.extend_from_slice(&pre_skip.to_le_bytes());
    head.extend_from_slice(&rate.to_le_bytes());
    head.extend_from_slice(&[0, 0, 0]);
    page(&mut out, 2, 0, &[&head]);
    let vendor = b"Vice Lens";
    let mut tags = b"OpusTags".to_vec();
    tags.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    tags.extend_from_slice(vendor);
    tags.extend_from_slice(&0u32.to_le_bytes());
    page(&mut out, 0, 0, &[&tags]);
    // Audio pages of up to 48 packets (well under 255 lacing values).
    let mut granule = pre_skip as u64;
    let groups: Vec<&[&[u8]]> = pkts.chunks(48).collect();
    for (k, g) in groups.iter().enumerate() {
        granule += g.iter().map(|p| opus_samples(p)).sum::<u64>();
        let last = k + 1 == groups.len();
        page(&mut out, if last { 4 } else { 0 }, granule, g);
    }
    out
}

impl Lens {
    /// Every bank in `audio.rpf`, found once: headers on 4 KiB boundaries,
    /// each bank stepped over whole.
    pub(super) fn banks(&self) -> Arc<Vec<Bank>> {
        self.banks
            .get_or_init(|| {
                let t = std::time::Instant::now();
                let mut out = Vec::new();
                let Some(a) = self.archive("audio.rpf") else { return Arc::new(out) };
                let b = a.bytes();
                let mut p = 0usize;
                while p + 16 <= b.len() {
                    if &b[p..p + 4] == b"ADAT" {
                        let header = u32le(b, p + 12) as usize;
                        if let Some(awc) = b.get(p..p + header.min(1 << 24)).and_then(parse) {
                            let bytes = extent(&awc);
                            if bytes > 0 && p as u64 + bytes as u64 <= b.len() as u64 {
                                out.push(Bank { offset: p as u64, bytes, streams: awc.streams.len() as u32, music: awc.flags & 0x0004_0000 != 0 });
                                p = (p + bytes as usize).div_ceil(4096) * 4096;
                                continue;
                            }
                        }
                    }
                    p += 4096;
                }
                log::info!("lens: {} sound banks in {:.1?}", out.len(), t.elapsed());
                Arc::new(out)
            })
            .clone()
    }

    pub(super) fn banks_list(&self, r: &Request) -> Reply {
        let banks = self.banks();
        let (from, limit) = (r.num::<usize>("from").unwrap_or(0), r.num::<usize>("limit").unwrap_or(500).min(5000));
        let music = r.get("music").map(|m| m == "1");
        let hit = |b: &&Bank| music.is_none_or(|m| b.music == m);
        let total = banks.iter().filter(hit).count();
        let items: Vec<String> =
            banks.iter().enumerate().filter(|(_, b)| hit(b)).skip(from).take(limit).map(|(k, b)| format!("[{k},\"{:#x}\",{},{},{}]", b.offset, b.bytes, b.streams, b.music)).collect();
        Reply::json(format!("{{\"total\":{total},\"all\":{},\"items\":[{}]}}", banks.len(), items.join(",")))
    }

    fn bank_bytes(&self, o: u64) -> Option<(Vec<u8>, Awc)> {
        let a = self.archive("audio.rpf")?;
        let bank = self.banks().iter().find(|b| b.offset == o).map(|b| b.bytes)?;
        let b = a.bytes().get(o as usize..o as usize + bank as usize)?.to_vec();
        let awc = parse(&b)?;
        Some((b, awc))
    }

    /// A bank's streams: id, sample rate, length, codec, packets.
    pub(super) fn bank(&self, r: &Request) -> Reply {
        let Some(o) = r.hex("o") else { return Reply::bad("o") };
        let Some((b, awc)) = self.bank_bytes(o) else { return Reply::missing("no bank there") };
        let music = awc.flags & 0x0004_0000 != 0;
        let items: Vec<String> = awc
            .streams
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let fmt = s.chunks.iter().find(|c| FORMAT.contains(&c.0)).and_then(|c| b.get(c.1 as usize..(c.1 + c.2) as usize)).filter(|f| f.len() >= 20);
                let data = s.chunks.iter().find(|c| c.0 == DATA);
                let (samples, rate, codec) = fmt.map_or((0, 0, 0), |f| (u32le(f, 0), u16le(f, 8) as u32, f[0x13]));
                let playable = !music && data.is_some() && fmt.is_some() && data.and_then(|c| b.get(c.1 as usize..(c.1 + c.2) as usize)).is_some_and(|d| !packets(d).is_empty());
                let seconds = if rate > 0 { samples as f32 / rate as f32 } else { 0.0 };
                format!("{{\"i\":{i},\"id\":\"{:08x}\",\"rate\":{rate},\"seconds\":{seconds:.2},\"codec\":{codec},\"bytes\":{},\"playable\":{playable}}}", s.id, data.map_or(0, |c| c.2))
            })
            .collect();
        Reply::json(format!("{{\"o\":\"{o:#x}\",\"music\":{music},\"items\":[{}]}}", items.join(",")))
    }

    /// One stream as Ogg Opus.
    pub(super) fn sound(&self, r: &Request) -> Reply {
        let (Some(o), Some(i)) = (r.hex("o"), r.num::<usize>("i")) else { return Reply::bad("o, i") };
        let Some((b, awc)) = self.bank_bytes(o) else { return Reply::missing("no bank there") };
        let Some(s) = awc.streams.get(i) else { return Reply::missing("no such stream") };
        let fmt = s.chunks.iter().find(|c| FORMAT.contains(&c.0)).and_then(|c| b.get(c.1 as usize..(c.1 + c.2) as usize)).filter(|f| f.len() >= 20);
        let Some(data) = s.chunks.iter().find(|c| c.0 == DATA).and_then(|c| b.get(c.1 as usize..(c.1 + c.2) as usize)) else { return Reply::missing("the stream has no data") };
        let pkts = packets(data);
        if pkts.is_empty() {
            return Reply::missing("no Opus packets in this stream");
        }
        let rate = fmt.map_or(48000, |f| u16le(f, 8) as u32).max(8000);
        let extra = fmt.filter(|f| f.len() >= 0x20).map_or(0, |f| u32le(f, 0x1c));
        let pre_skip = if extra < 10_000 { extra as u16 } else { 0 };
        let channels = if pkts[0][0] & 4 != 0 { 2 } else { 1 };
        let ogg = ogg_opus(&pkts, channels, rate, pre_skip);
        let mut reply = Reply::bytes("audio/ogg", ogg, true);
        reply.extra.push(("Accept-Ranges", "none".into()));
        reply
    }
}

//! Bink 2 movies: the header, the audio tracks, and one track's packets
//! repacked as a Bink Audio 2 stream ("UEBA"), which FFmpeg's `binka`
//! demuxer reads and decodes as the second version of Bink Audio. FFmpeg's
//! Bink demuxer hands movie audio to the decoder as the first version,
//! which turns the game's tracks into full-scale noise.
//!
//! The header: tag (`KB2n`), size - 8, frames, largest frame, -, width,
//! height, fps, fps divisor, video flags, audio tracks; one more word from
//! revision `i` of Bink 2; then per track the largest decoded size, then
//! per track rate (u16) and flags (u16: 0x2000 stereo, 0x1000 DCT), then
//! per track an id; then the frame offsets (bit 0: key frame). A frame
//! starts with each track's packet (u32 size; inside, u32 decoded bytes
//! and the bitstream), then the picture.

/// An audio track of a movie.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub id: u32,
    pub rate: u32,
    pub channels: u8,
}

/// What Vice Lens reads of a movie's header.
#[derive(Debug)]
pub struct Header {
    pub tracks: Vec<Track>,
    /// Where each frame starts, and where the last one ends.
    pub frames: Vec<usize>,
}

fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    b.get(o..o + 4).map(|s| u32::from_le_bytes(s.try_into().unwrap()))
}

/// The header of a Bink movie held whole in `b`.
pub fn header(b: &[u8]) -> Option<Header> {
    let (tag, rev) = (b.get(0..3)?, *b.get(3)?);
    let bink2 = tag == b"KB2";
    if !bink2 && tag != b"BIK" {
        return None;
    }
    let frames = u32_at(b, 8)? as usize;
    let n = u32_at(b, 40)? as usize;
    if n > 256 || frames == 0 || frames > 1_000_000 {
        return None;
    }
    let mut p = 44 + if (bink2 && rev >= b'i') || (!bink2 && rev == b'k') { 4 } else { 0 };
    p += 4 * n;
    let mut tracks = Vec::with_capacity(n);
    for i in 0..n {
        let w = u32_at(b, p + 4 * i)?;
        tracks.push(Track { id: 0, rate: w & 0xffff, channels: if (w >> 16) & 0x2000 != 0 { 2 } else { 1 } });
    }
    p += 4 * n;
    for (i, t) in tracks.iter_mut().enumerate() {
        t.id = u32_at(b, p + 4 * i)?;
    }
    p += 4 * n;
    let end = (u32_at(b, 4)? as usize + 8).min(b.len());
    let mut at = Vec::with_capacity(frames + 1);
    for i in 0..frames {
        at.push(u32_at(b, p + 4 * i)? as usize & !1);
    }
    at.push(end);
    if at.windows(2).any(|w| w[1] <= w[0]) {
        return None;
    }
    Some(Header { tracks, frames: at })
}

/// Track `track` of the movie in `b` as a UEBA stream; None when the movie
/// has no such track or a packet does not fit the format.
pub fn ueba(b: &[u8], track: usize) -> Option<Vec<u8>> {
    let h = header(b)?;
    let t = h.tracks.get(track)?;
    let mut packets = Vec::new();
    let mut decoded = 0u64;
    for f in h.frames.windows(2) {
        let mut q = f[0];
        for k in 0..h.tracks.len() {
            let size = u32_at(b, q)? as usize;
            q += 4;
            if q + size > f[1] {
                return None;
            }
            if k == track && size >= 4 {
                decoded += u32_at(b, q)? as u64;
                packets.push(&b[q + 4..q + size]);
            }
            q += size;
        }
    }
    if packets.is_empty() {
        return None;
    }
    let samples = (decoded / (2 * t.channels as u64)) as u32;
    let mut out = Vec::with_capacity(28 + packets.iter().map(|p| p.len() + 4).sum::<usize>());
    out.extend_from_slice(b"ABEU");
    out.extend_from_slice(&[1, t.channels, 0, 0]);
    out.extend_from_slice(&t.rate.to_le_bytes());
    out.extend_from_slice(&samples.to_le_bytes());
    out.extend_from_slice(&[0; 8]);
    // No seek table: its entry count, then two bytes the reader skips.
    out.extend_from_slice(&[0; 4]);
    for p in packets {
        let len = u16::try_from(p.len()).ok()?;
        out.extend_from_slice(&0x9999u16.to_le_bytes());
        if len == 0xffff {
            out.extend_from_slice(&[0xff, 0xff]);
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
        } else {
            out.extend_from_slice(&len.to_le_bytes());
        }
        out.extend_from_slice(p);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A two-frame KB2n movie with two stereo tracks; track 1 has a packet
    /// only in the first frame.
    fn movie() -> Vec<u8> {
        let w = |v: &mut Vec<u8>, x: u32| v.extend_from_slice(&x.to_le_bytes());
        let mut b = b"KB2n".to_vec();
        for x in [0, 2, 100, 2, 64, 64, 30, 1, 0, 2, 0] {
            w(&mut b, x);
        }
        w(&mut b, 4000);
        w(&mut b, 4000);
        w(&mut b, 48000 | 0x7000 << 16);
        w(&mut b, 44100 | 0x3000 << 16);
        w(&mut b, 3);
        w(&mut b, 1);
        let index = b.len();
        w(&mut b, 0);
        w(&mut b, 0);
        let f0 = b.len();
        // Frame 0: track 0 (decoded 16 bytes, data 1 2 3), track 1 (8, data 9), picture.
        w(&mut b, 7);
        w(&mut b, 16);
        b.extend_from_slice(&[1, 2, 3]);
        w(&mut b, 5);
        w(&mut b, 8);
        b.push(9);
        // Frames start on even offsets (bit 0 of an offset marks a key frame).
        b.extend_from_slice(&[0xee; 4]);
        let f1 = b.len();
        // Frame 1: track 0 (decoded 4 bytes, data 4), track 1 empty, picture.
        w(&mut b, 5);
        w(&mut b, 4);
        b.push(4);
        w(&mut b, 0);
        b.extend_from_slice(&[0xee; 3]);
        let len = b.len() as u32 - 8;
        b[4..8].copy_from_slice(&len.to_le_bytes());
        b[index..index + 4].copy_from_slice(&(f0 as u32 | 1).to_le_bytes());
        b[index + 4..index + 8].copy_from_slice(&(f1 as u32).to_le_bytes());
        b
    }

    #[test]
    fn reads_tracks() {
        let h = header(&movie()).unwrap();
        assert_eq!(h.tracks, vec![Track { id: 3, rate: 48000, channels: 2 }, Track { id: 1, rate: 44100, channels: 2 }]);
        assert_eq!(h.frames.len(), 3);
    }

    #[test]
    fn repacks_a_track() {
        let u = ueba(&movie(), 0).unwrap();
        assert_eq!(&u[..8], b"ABEU\x01\x02\x00\x00");
        assert_eq!(u32_at(&u, 8), Some(48000));
        // 20 decoded bytes of 16-bit stereo: 5 samples.
        assert_eq!(u32_at(&u, 12), Some(5));
        assert_eq!(&u[28..], &[0x99, 0x99, 3, 0, 1, 2, 3, 0x99, 0x99, 1, 0, 4]);
        let u = ueba(&movie(), 1).unwrap();
        assert_eq!(&u[28..], &[0x99, 0x99, 1, 0, 9]);
        assert!(ueba(&movie(), 2).is_none());
    }
}

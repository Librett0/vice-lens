//! Readers for the game's data formats.
//!
//! Everything here reads the original files in place and never writes to them.
//! See `docs/formats.md` for what each format looks like.

pub mod anim;
pub mod archive;
pub mod drawable;
pub mod gfx;
pub mod hash;
pub mod index;
pub mod mapdata;
pub mod navmesh;
pub mod pso;
pub mod rbf;
pub mod rsc;
pub mod swizzle;
pub mod text;
pub mod ttpf;
pub mod txd;

pub use archive::{Archive, Stream};
pub use hash::joaat;

/// Little-endian u64 at `o` (0 when out of range).
pub fn le_u64(b: &[u8], o: usize) -> u64 {
    le::u64(b, o)
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Format(String),
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn bad<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Format(msg.into()))
}

/// Little-endian readers over a byte slice; out-of-range reads return 0 so
/// that callers validate structure instead of panicking on hostile data.
pub mod le {
    pub fn u8(b: &[u8], o: usize) -> u8 {
        b.get(o).copied().unwrap_or(0)
    }
    pub fn u16(b: &[u8], o: usize) -> u16 {
        b.get(o..o + 2).map_or(0, |s| u16::from_le_bytes([s[0], s[1]]))
    }
    pub fn i16(b: &[u8], o: usize) -> i16 {
        u16(b, o) as i16
    }
    pub fn u32(b: &[u8], o: usize) -> u32 {
        b.get(o..o + 4).map_or(0, |s| u32::from_le_bytes(s.try_into().unwrap()))
    }
    pub fn u64(b: &[u8], o: usize) -> u64 {
        b.get(o..o + 8).map_or(0, |s| u64::from_le_bytes(s.try_into().unwrap()))
    }
    pub fn i64(b: &[u8], o: usize) -> i64 {
        u64(b, o) as i64
    }
    pub fn f64(b: &[u8], o: usize) -> f64 {
        f64::from_bits(u64(b, o))
    }
    /// NUL-terminated string starting at `o`.
    pub fn cstr(b: &[u8], o: usize) -> &[u8] {
        let tail = b.get(o..).unwrap_or(&[]);
        let end = tail.iter().position(|&c| c == 0).unwrap_or(tail.len());
        &tail[..end]
    }
}

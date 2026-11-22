//! Jenkins one-at-a-time, the string hash used throughout the data.

/// `joaat` over the ASCII-lowercased bytes of `s`, as the game hashes names
/// and text labels.
pub fn joaat(s: &str) -> u32 {
    joaat_bytes(s.as_bytes())
}

pub fn joaat_bytes(s: &[u8]) -> u32 {
    let mut h: u32 = 0;
    for &c in s {
        h = h.wrapping_add(c.to_ascii_lowercase() as u32);
        h = h.wrapping_add(h << 10);
        h ^= h >> 6;
    }
    h = h.wrapping_add(h << 3);
    h ^= h >> 11;
    h.wrapping_add(h << 15)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_label_hashes() {
        // Keys found in the English text table for these pause menu labels.
        assert_eq!(joaat("PM_INDX_GAME"), 0xa08a_bcf1);
        assert_eq!(joaat("SCROLL_TIP"), 0xd808_d082);
        assert_eq!(joaat("pm_indx_game"), joaat("PM_INDX_GAME"));
    }
}

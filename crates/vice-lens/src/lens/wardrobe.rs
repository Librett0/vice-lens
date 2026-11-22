//! Vice Lens: the wardrobe. People are dressed from separate models, each
//! in the same bind pose, so drawn together they make a person: a head
//! (`head_mr1_cau_074_m_eyes_full`), hair, beard and brows, and clothes and
//! accessories named `<item>_<body>_<nnn>_<details>_full` for each body
//! (`mr1` men, `fr1` women, `mb1`/`fb1`, `mo1`/`fo1`, children `mk1`, `mc1`,
//! `mi1`, `fc1`). Named characters carry their own set under one prefix:
//! `cs_<name>_…` (cutscenes), `player_story000_<nn>_…` (the protagonists'
//! outfits), `hc_<group>_head_…` (gang and crowd faces). `_cull` versions
//! (skin hidden under other clothes), `_alt_nn`, first-person `_fp` and
//! torn `_aNN` versions are left out.

use std::collections::BTreeMap;

use super::Lens;
use super::http::{Reply, quote};

const BODIES: [&str; 10] = ["mr1", "fr1", "mb1", "fb1", "mo1", "fo1", "mk1", "mc1", "mi1", "fc1"];

/// What a kind of item is, by the start of its name, in the wardrobe's
/// slots (one item per slot is worn).
const SLOTS: [(&str, &[&str]); 27] = [
    ("Head", &["head"]),
    ("Eyes", &[]),
    ("Eye surface", &[]),
    ("Body", &["hand", "feet", "bodyuppergen", "bodylowergen"]),
    ("Mannequin", &["mannequin"]),
    ("Whole person (older style)", &["legacypedworld"]),
    ("Hair", &["hair", "wigs"]),
    ("Facial hair", &["beard", "mustache", "chin", "chops", "stubble"]),
    ("Brows and lashes", &["brows", "lashes", "eyes", "teeth", "fingernails"]),
    ("Body hair", &["bodyhair", "chesthair", "leghair", "armpithair", "genitalhair"]),
    ("Underwear", &["undies", "bra", "basetops", "swimsuit"]),
    ("Top", &["tshirt", "shirt", "tanktop", "sweatshirt", "hoodie", "vest", "blouse", "crop", "bodice"]),
    ("Jacket", &["jacket", "coat", "gilet", "cape"]),
    ("Dress and full body", &["dress", "fullbody", "costume", "apron", "skirt"]),
    ("Legs", &["pants", "shorts", "jeans", "leggings", "basebottoms"]),
    ("Socks", &["socks"]),
    ("Shoes", &["shoes", "boots", "sandals", "heels"]),
    ("Hat", &["hat", "helmet", "cap", "bandanahead", "fitted", "proxycap"]),
    ("Glasses", &["shades", "specs", "goggles"]),
    ("Mask", &["masks", "wrap"]),
    ("Neck", &["necklace", "neckwear", "towelneck"]),
    ("Ears and piercings", &["piercing", "headphones", "bodypiercings"]),
    ("Wrists and hands", &["watch", "bracelet", "rings", "gloves", "s01_watch"]),
    ("Bags", &["bag", "backpack"]),
    ("Belts and holsters", &["belt", "holsters", "gunstraps", "ankle", "badge", "patch", "pins", "slings"]),
    ("Armour", &["armor"]),
    ("Other", &[""]),
];

fn slot(item: &str) -> &'static str {
    SLOTS.iter().find(|(_, starts)| starts.iter().any(|s| item.starts_with(s))).map_or("Other", |s| s.0)
}

/// A head comes in pieces worn together: the face, the eyes, the wet
/// layer over them and the lashes.
fn head_slot(rest: &str) -> &'static str {
    if rest.contains("_eyecap") {
        "Eye surface"
    } else if rest.contains("_eyelashes") {
        "Brows and lashes"
    } else if rest.contains("_eyes") {
        "Eyes"
    } else {
        "Head"
    }
}

/// An item's name split: (character prefix, item kind, body, rest).
fn split(name: &str) -> Option<(&str, &str, &str, &str)> {
    // Bare hands and feet come only as `_cull` versions; they are the body.
    let bare = (name.starts_with("hand_") || name.starts_with("feet_")) && name.ends_with("_nude_cull");
    let skip = name.ends_with("_cull") || name.contains("_cull_") || name.contains("_alt_") || name.ends_with("_fp") || name.contains("tattooprojection");
    // Names the index read with stray bytes before them (`Bshades…`), and
    // collision shapes, gore and name tags, are not items.
    if (skip && !bare) || !name.starts_with(|c: char| c.is_ascii_lowercase()) || ["char_", "slod_", "gore", "tagname"].iter().any(|p| name.starts_with(p)) {
        return None;
    }
    let toks: Vec<&str> = name.split('_').collect();
    let bi = toks.iter().position(|t| BODIES.contains(t))?;
    if bi == 0 {
        return None;
    }
    // A torn or rolled-up version (`_a02`) is not an item of its own.
    if toks[bi + 1..].iter().any(|t| t.len() == 3 && t.starts_with('a') && t[1..].bytes().all(|c| c.is_ascii_digit())) {
        return None;
    }
    let kind_at = bi - 1;
    let prefix_len: usize = toks[..kind_at].iter().map(|t| t.len() + 1).sum();
    let kind_start = prefix_len;
    let body_start = kind_start + toks[kind_at].len() + 1;
    let prefix = name[..prefix_len].trim_end_matches('_');
    // A head with a character's name: `hc_colombians_head_…`, `cs_x_head_…`.
    Some((prefix, &name[kind_start..body_start - 1], toks[bi], &name[body_start + toks[bi].len()..]))
}

impl Lens {
    /// Every item: clothes and accessories by slot and body, and the named
    /// characters' sets.
    pub(super) fn wardrobe(&self) -> Reply {
        let Some(w) = self.the_world() else { return Reply::missing("no model index") };
        let mut items: Vec<String> = Vec::new();
        let mut people: BTreeMap<String, (String, Vec<String>)> = BTreeMap::new();
        for (h, n) in &w.index.names {
            let Some((prefix, kind, body, rest)) = split(n) else { continue };
            let slot = if kind == "head" { head_slot(rest) } else { slot(kind) };
            let entry = format!("{{\"h\":\"{h:08x}\",\"name\":{},\"kind\":{},\"slot\":{},\"body\":\"{body}\",\"rest\":{}}}", quote(n), quote(kind), quote(slot), quote(rest.trim_start_matches('_')));
            // Cutscene characters, the protagonists' outfits, crowd faces,
            // and the special ones (`sc_alien`, `sc_boss_demon`,
            // `u_m_m_farm_monster_man`) are people; other prefixes
            // (`armor_torso_body_armor`) are part of an item's name.
            if prefix.is_empty() || !["cs_", "hc_", "player_story", "sc_", "u_m_", "u_f_"].iter().any(|p| prefix.starts_with(p)) {
                items.push(entry);
            } else {
                let p = people.entry(prefix.to_string()).or_insert_with(|| (body.to_string(), Vec::new()));
                if slot == "Head" {
                    p.0 = body.to_string();
                }
                p.1.push(entry);
            }
        }
        items.sort();
        for p in people.values_mut() {
            p.1.sort();
        }
        let people: Vec<String> = people.iter().map(|(name, (body, list))| format!("{{\"name\":{},\"body\":\"{body}\",\"items\":[{}]}}", quote(name), list.join(","))).collect();
        let slots: Vec<String> = SLOTS.iter().map(|s| quote(s.0)).collect();
        Reply::json(format!("{{\"slots\":[{}],\"items\":[{}],\"people\":[{}]}}", slots.join(","), items.join(","), people.join(",")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_names_split_into_kind_body_and_character() {
        assert_eq!(split("tshirtcrewneck_mr1_000_slim_cc_tu_long_full"), Some(("", "tshirtcrewneck", "mr1", "_000_slim_cc_tu_long_full")));
        assert_eq!(split("cs_roxy_hairfrenchbraid_fr1_000_curly_l08_non_sm_p_full").map(|s| (s.0, s.1)), Some(("cs_roxy", "hairfrenchbraid")));
        assert_eq!(split("hc_colombians_head_mr1_lat_000_m").map(|s| (s.0, s.1)), Some(("hc_colombians", "head")));
        assert_eq!(split("player_story000_01_pantsjeans_mr1_000_slim_mw_long_full").map(|s| (s.0, s.1, s.2)), Some(("player_story000_01", "pantsjeans", "mr1")));
        // Bare feet and hands are the body; a culled piece of clothing is
        // not an item of its own.
        assert_eq!(split("feet_mr1_000_nude_cull").map(|s| s.1), Some("feet"));
        assert_eq!(split("tshirtcrewneck_mr1_000_slim_cc_tu_long_cull_00"), None);
        assert_eq!(head_slot("_lat_000_m_eyes_full"), "Eyes");
        assert_eq!(head_slot("_lat_000_m_eyecap"), "Eye surface");
        assert_eq!(head_slot("_lat_000_m"), "Head");
        assert_eq!(split("hand_mr1_000_tshirt_a02"), None);
        assert_eq!(split("char_bipedbounds_male_tshirtcrewneck_mr1_000_x"), None);
        assert_eq!(slot("tshirtcrewneck"), "Top");
        assert_eq!(slot("shoessneakers"), "Shoes");
        assert_eq!(slot("head"), "Head");
        assert_eq!(slot("beardgoatee"), "Facial hair");
    }
}

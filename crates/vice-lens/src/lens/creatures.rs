//! Vice Lens: the animals and other creatures. Each is a set of models in
//! one bind pose, named `<class>_<species>_<sex>_<nn>_<part>`: the class
//! `crm` mammals, `crb` birds, `crr` reptiles, `crf` fish, `cra` amphibians,
//! `crc` crustaceans, `cri` insects; parts such as `body_00`, `head_00`,
//! `eyes_00`, `teeth_00`, `fur_body_medium_00`, `feather_00`, and looks
//! of named animals (`body_03_missmojo`, `costume_03_missmojo`).

use std::borrow::Cow;
use std::collections::{BTreeMap, HashSet};

use super::Lens;
use super::http::{Reply, quote};

/// What the class letters mean.
pub(super) const CLASSES: [(&str, &str); 8] =
    [("crm", "Mammals"), ("crb", "Birds"), ("crr", "Reptiles"), ("crf", "Fish"), ("cra", "Amphibians"), ("crc", "Crustaceans"), ("cri", "Insects"), ("crs", "Others")];

/// A creature model's name split: (animal, part). The animal is everything
/// up to and with the sex and number (`crm_dog_chihuahua_f_00`). Some
/// animals' own parts leave the number out (`crm_dog_carolina_m_body_00`
/// of `crm_dog_carolina_m_00`): those name the sex's animal `00`.
fn split(name: &str) -> Option<(Cow<'_, str>, &str)> {
    if !name.starts_with("cr") || name.as_bytes().get(3) != Some(&b'_') {
        return None;
    }
    let toks: Vec<&str> = name.split('_').collect();
    let sex = |t: &str| t == "m" || t == "f" || t == "u";
    let num = |t: &str| t.len() == 2 && t.bytes().all(|c| c.is_ascii_digit());
    // The length of the first `k` tokens with their underscores.
    let len = |k: usize| toks[..k].iter().map(|t| t.len() + 1).sum::<usize>() - 1;
    // The sex (m, f) followed by a two-digit number ends the animal.
    if let Some(at) = (2..toks.len().saturating_sub(1)).find(|&i| sex(toks[i]) && num(toks[i + 1])) {
        let end = len(at + 2);
        return Some((Cow::Borrowed(&name[..end]), name.get(end + 1..).unwrap_or("")));
    }
    let at = (2..toks.len().saturating_sub(1)).find(|&i| sex(toks[i]))?;
    let end = len(at + 1);
    Some((Cow::Owned(format!("{}_00", &name[..end])), &name[end + 1..]))
}

/// A part's slot and look: `fur_body_medium_00` → ("fur body", "medium");
/// `body_03_missmojo` → ("body", "missmojo"); `eyes_00` → ("eyes", "").
fn slot_look(part: &str) -> (String, String) {
    let toks: Vec<&str> = part.split('_').collect();
    let two = toks.len() > 1 && ["fur", "generic", "feather", "feathers", "hair", "mane"].contains(&toks[0]) && !toks[1].bytes().all(|c| c.is_ascii_digit());
    let n = if two { 2 } else { 1 };
    let slot = toks[..n.min(toks.len())].join(" ");
    let look: Vec<&str> = toks[n.min(toks.len())..].iter().filter(|t| !t.bytes().all(|c| c.is_ascii_digit())).copied().collect();
    (slot, look.join(" "))
}

impl Lens {
    /// Every creature: its class, its parts by slot, and its looks.
    pub(super) fn creatures(&self) -> Reply {
        let Some(w) = self.the_world() else { return Reply::missing("no model index") };
        // The animals: each has a model of its bare name (a placeholder of
        // its bounds, which holds its skeleton).
        let bare: HashSet<&str> = w.index.names.values().map(|n| n.as_str()).filter(|n| split(n).is_some_and(|(_, p)| p.is_empty())).collect();
        let mut animals: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (h, n) in &w.index.names {
            let Some((animal, part)) = split(n) else { continue };
            // A part without the animal's number goes to an animal that is.
            if part.is_empty() || (matches!(animal, Cow::Owned(_)) && !bare.contains(animal.as_ref())) {
                continue;
            }
            let (slot, look) = slot_look(part);
            animals.entry(animal.into_owned()).or_default().push(format!("{{\"h\":\"{h:08x}\",\"name\":{},\"slot\":{},\"look\":{}}}", quote(n), quote(&slot), quote(&look)));
        }
        let items: Vec<String> = animals
            .iter_mut()
            .map(|(a, parts)| {
                parts.sort();
                let class = CLASSES.iter().find(|c| a.starts_with(c.0)).map_or("Others", |c| c.1);
                format!("{{\"name\":{},\"class\":\"{class}\",\"parts\":[{}]}}", quote(a), parts.join(","))
            })
            .collect();
        Reply::json(format!("{{\"items\":[{}]}}", items.join(",")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creature_names_split_into_animal_part_slot_and_look() {
        let s = |n| split(n).map(|(a, p)| (a.into_owned(), p));
        assert_eq!(s("crm_dog_chihuahua_f_00_fur_body_medium_00"), Some(("crm_dog_chihuahua_f_00".into(), "fur_body_medium_00")));
        assert_eq!(s("crb_dodo_generic_f_01_feather_00"), Some(("crb_dodo_generic_f_01".into(), "feather_00")));
        assert_eq!(s("crm_boar_wild_m_00"), Some(("crm_boar_wild_m_00".into(), "")));
        // A part that leaves the animal's number out.
        assert_eq!(s("crm_dog_carolina_m_body_00"), Some(("crm_dog_carolina_m_00".into(), "body_00")));
        assert_eq!(s("crm_cat_shorthairamerican_m_head_12_discoaf"), Some(("crm_cat_shorthairamerican_m_00".into(), "head_12_discoaf")));
        assert_eq!(s("crane_01"), None);
        assert_eq!(slot_look("fur_body_medium_00"), ("fur body".to_string(), "medium".to_string()));
        assert_eq!(slot_look("body_03_missmojo"), ("body".to_string(), "missmojo".to_string()));
        assert_eq!(slot_look("eyes_00"), ("eyes".to_string(), String::new()));
        assert_eq!(slot_look("feather_00"), ("feather".to_string(), String::new()));
    }
}

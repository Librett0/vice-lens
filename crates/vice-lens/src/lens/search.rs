//! Vice Lens: how a search matches, shared by every list (models, text,
//! textures, archive files), and the game's text: the interface table and
//! the subtitles.
//!
//! A query matches as plain text, ignoring case, unless the request asks
//! otherwise: `case=1` matches case, `word=1` whole words only, `re=1`
//! reads the query as a regular expression.

use std::collections::HashMap;

use rage::Archive;

use super::http::Request;

pub(super) struct Matcher {
    re: Option<regex::Regex>,
}

impl Matcher {
    /// The matcher for the request's `q` and options; an empty query
    /// matches everything. An invalid regular expression is an error the
    /// page shows.
    pub fn from(r: &Request) -> Result<Matcher, String> {
        let q = r.get("q").unwrap_or("").trim();
        Self::new(q, r.get("case") == Some("1"), r.get("word") == Some("1"), r.get("re") == Some("1"))
    }

    pub fn new(q: &str, case: bool, word: bool, re: bool) -> Result<Matcher, String> {
        if q.is_empty() {
            return Ok(Matcher { re: None });
        }
        let body = if re { q.to_string() } else { regex::escape(q) };
        let body = if word { format!(r"\b(?:{body})\b") } else { body };
        regex::RegexBuilder::new(&body)
            .case_insensitive(!case)
            .size_limit(1 << 22)
            .build()
            .map(|re| Matcher { re: Some(re) })
            .map_err(|e| format!("not a regular expression: {}", e.to_string().lines().last().unwrap_or("")))
    }

    pub fn all(&self) -> bool {
        self.re.is_none()
    }

    pub fn is_match(&self, s: &str) -> bool {
        self.re.as_ref().is_none_or(|re| re.is_match(s))
    }

    /// Where the first match starts (0 for an empty query), to rank names
    /// that start with the query first.
    pub fn start(&self, s: &str) -> Option<usize> {
        match &self.re {
            None => Some(0),
            Some(re) => re.find(s).map(|m| m.start()),
        }
    }
}

/// The game's text: the interface table and the subtitles (lines spoken in
/// cutscenes, phone calls and on the street), English.
pub(super) struct Text {
    /// (label hash, text, source): source 0 is the interface, n > 0 the
    /// subtitle table n.
    pub lines: Vec<(u32, String, u32)>,
    /// Subtitle tables: where each lies in data.rpf.
    pub tables: Vec<u64>,
}

/// Load the text. The copy holds the interface in eight languages (told
/// apart by how they spell "Settings") and every subtitle table in several:
/// translations of one table share its labels, so tables are grouped by
/// their labels and the English one of each group picked by its words.
/// The English tables found are kept in the cache, so later runs read only
/// those.
pub(super) fn load(lens: &super::Lens) -> Text {
    let mut out = Text { lines: Vec::new(), tables: Vec::new() };
    let Some(a) = lens.archive("data.rpf") else { return out };
    let list = super::cache().join("text-subtitles.txt");
    let known: Option<(u64, Vec<u64>)> = std::fs::read_to_string(&list).ok().and_then(|s| {
        let mut it = s.lines().filter_map(|l| u64::from_str_radix(l.trim().trim_start_matches("0x"), 16).ok());
        Some((it.next()?, it.collect()))
    });
    let (ui, subs) = match known {
        Some(k) => k,
        None => {
            let found = scan(&a);
            let body: Vec<String> = std::iter::once(found.0).chain(found.1.iter().copied()).map(|o| format!("{o:#x}")).collect();
            let _ = std::fs::write(&list, body.join("\n"));
            found
        }
    };
    let read = |o: u64| a.file_at(o).ok().and_then(|d| rage::text::parse(&d).ok());
    if let Some(t) = read(ui) {
        out.lines.extend(t.iter().map(|(h, s)| (h, s.to_string(), 0)));
    }
    for (k, &o) in subs.iter().enumerate() {
        if let Some(t) = read(o) {
            let mut v: Vec<(u32, String, u32)> = t.iter().map(|(h, s)| (h, s.to_string(), k as u32 + 1)).collect();
            v.sort_by_key(|x| x.0);
            out.lines.extend(v);
        }
        out.tables.push(o);
    }
    log::info!("lens: {} lines of text ({} subtitle tables)", out.lines.len(), subs.len());
    out
}

/// Find the English interface table and the English subtitle tables.
fn scan(a: &Archive) -> (u64, Vec<u64>) {
    let mut streams = Vec::new();
    a.scan(|s, _| {
        streams.push(s);
        true
    });
    let mut ui = 0;
    // Label set → (best English score, table).
    let mut groups: HashMap<u64, (f32, u64)> = HashMap::new();
    for f in Archive::files(&streams) {
        let Ok(d) = a.read_file(&f) else { continue };
        if !rage::text::is_text_table(&d) {
            continue;
        }
        let Ok(t) = rage::text::parse(&d) else { continue };
        if t.get("PM_INDX_SET").is_some() {
            if t.get("PM_INDX_SET") == Some("Settings") {
                ui = f.offset();
            }
            continue;
        }
        let mut labels: Vec<u32> = t.iter().map(|x| x.0).collect();
        labels.sort_unstable();
        let set = labels.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &l| (h ^ l as u64).wrapping_mul(0x100_0000_01b3));
        let score = english(t.iter().map(|x| x.1));
        let g = groups.entry(set).or_insert((f32::MIN, 0));
        if score > g.0 {
            *g = (score, f.offset());
        }
    }
    let mut subs: Vec<u64> = groups.into_values().filter(|g| g.0 > 0.02).map(|g| g.1).collect();
    subs.sort_unstable();
    (ui, subs)
}

/// How English a table reads: the share of its words that are "the",
/// "you", "and" or "is", nothing if many letters lie outside ASCII.
fn english<'a>(lines: impl Iterator<Item = &'a str>) -> f32 {
    let (mut words, mut common, mut chars, mut wide) = (0usize, 0usize, 0usize, 0usize);
    for l in lines {
        chars += l.len();
        wide += l.chars().filter(|c| !c.is_ascii() && *c != '’' && *c != '…' && *c != '♪').count();
        for w in l.split(|c: char| !c.is_ascii_alphabetic() && c != '\'') {
            if w.is_empty() {
                continue;
            }
            words += 1;
            if ["the", "you", "and", "is"].iter().any(|c| w.eq_ignore_ascii_case(c)) {
                common += 1;
            }
        }
    }
    if words == 0 || wide * 100 > chars.max(1) {
        return 0.0;
    }
    common as f32 / words as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_queries_ignore_case_and_options_narrow_them() {
        let m = Matcher::new("Ape", false, false, false).unwrap();
        assert!(m.is_match("skunk ape crossing"));
        let m = Matcher::new("Ape", true, false, false).unwrap();
        assert!(!m.is_match("skunk ape crossing"));
        let m = Matcher::new("ape", false, true, false).unwrap();
        assert!(m.is_match("skunk ape crossing") && !m.is_match("grapes"));
        // Plain text is not read as a pattern; a pattern is when asked.
        assert!(!Matcher::new("a.e", false, false, false).unwrap().is_match("ape"));
        assert!(Matcher::new("^prop_.*_0[0-9]a$", false, false, true).unwrap().is_match("prop_am_bin_04a"));
        assert!(Matcher::new("(", false, false, true).is_err());
        assert_eq!(Matcher::new("bin", false, false, false).unwrap().start("prop_am_bin"), Some(8));
    }

    #[test]
    fn english_tables_read_as_english() {
        assert!(english(["You know you're not supposed to be here, right?", "Is the door open and the car ready?"].into_iter()) > 0.1);
        assert_eq!(english(["Vous savez que vous ne devriez pas être ici, n'est-ce pas ?"].into_iter()), 0.0);
        assert!(english(["Sabes que no deberías estar aquí."].into_iter()) < 0.02);
    }
}

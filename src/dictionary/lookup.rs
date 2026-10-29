use std::collections::{HashMap, HashSet};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::Response;

use crate::editor_core::format_control::is_format_control;
use crate::editor_core::word_boundary::Script;

/// Maximum number of suggestions `suggest()` ever returns (Phase P9,
/// `WORD_SUGGESTIONS_PLAN.md` §5).
const SUGGESTION_LIMIT: usize = 8;

/// Upper bound on how many raw prefix matches `suggest()` scans before
/// ranking/deduplicating, independent of `SUGGESTION_LIMIT` - a short,
/// common prefix could otherwise match thousands of entries before
/// we've even had a chance to rank and cut down to `SUGGESTION_LIMIT`.
const MAX_PREFIX_SCAN: usize = 500;

/// Maximum number of Cyrillic candidates `reverse_lookup()` returns
/// (`prompt/WORD_TRANSLATION_PLAN.md`).
const TRANSLATION_LIMIT: usize = 6;

/// Cyrillic -> Traditional Mongolian dictionary.
///
/// Keys are the Cyrillic column (lower-cased, trimmed). Each key maps to one
/// or more Mongolian traditional variants (duplicates in the source TSV are
/// preserved as separate entries in the `Vec`).
pub struct Dictionary {
    map: HashMap<String, Vec<String>>,
    /// Single-word `(cyrillic, mongolian)` pairs only - phrase entries
    /// (either column containing whitespace) are excluded from the
    /// suggestion indices below, per `WORD_SUGGESTIONS_PLAN.md` §5 (they
    /// are ~75% of the source TSV and would dominate/degrade live word
    /// completion; `lookup()` above is unaffected and still sees every
    /// entry, phrases included). Built once by `build_indices()`.
    entries: Vec<(String, String)>,
    /// `(lowercased cyrillic key, index into `entries`)`, sorted by key -
    /// queried by `suggest(.., Script::Cyrillic, ..)` via prefix range.
    cyrillic_index: Vec<(String, usize)>,
    /// `(format-control-stripped mongolian value, index into `entries`)`,
    /// sorted by key - queried by `suggest(.., Script::Mongolian, ..)`.
    /// Stripped rather than raw, since sorting/prefix-matching strings
    /// with invisible modifier characters interleaved would not match
    /// what a user typed (which also goes through the same stripping
    /// via `Dictionary::normalize_mongolian`).
    mongolian_index: Vec<(String, usize)>,
    /// Format-control-stripped Mongolian value -> Cyrillic keys that
    /// map to it, for `reverse_lookup()` (double-click translation).
    /// Unlike the suggestion indices, rows whose *Cyrillic* side is a
    /// phrase are kept (a single Bichig word can legitimately translate
    /// to a multi-word Cyrillic phrase); rows whose Bichig side contains
    /// whitespace are skipped, since a double-clicked word can never
    /// match them.
    reverse_map: HashMap<String, Vec<String>>,
}

impl Dictionary {
    /// Fetches the prepared TSV file and parses it. Safe to call once at
    /// startup or lazily on first use.
    pub async fn load(path: &str) -> Result<Self, JsValue> {
        let window = web_sys::window().ok_or("No window")?;
        let resp_value = JsFuture::from(window.fetch_with_str(path)).await?;
        let resp: Response = resp_value.dyn_into()?;
        if !resp.ok() {
            return Err(JsValue::from_str(&format!(
                "Failed to fetch dictionary: HTTP {}",
                resp.status()
            )));
        }
        let text_promise = resp.text()?;
        let text_value = JsFuture::from(text_promise).await?;
        let text = text_value
            .as_string()
            .ok_or("Failed to read dictionary as string")?;

        Ok(Self::from_tsv(&text))
    }

    /// Parses the TSV text into an in-memory HashMap. Exposed separately so
    /// unit tests (added in a future pass) can cover it without the fetch.
    pub fn from_tsv(text: &str) -> Self {
        let mut map: HashMap<String, Vec<String>> = HashMap::new();

        for line in text.lines() {
            // Skip blank lines and comments.
            if line.is_empty() {
                continue;
            }
            if line.starts_with('#') {
                continue;
            }

            // Expect exactly one tab separator.
            let mut parts = line.splitn(2, '\t');
            let cyrillic = match parts.next() {
                Some(s) => s.trim(),
                None => continue,
            };
            let mongolian = match parts.next() {
                Some(s) => s.trim(),
                None => continue,
            };

            if cyrillic.is_empty() || mongolian.is_empty() {
                continue;
            }

            let key = cyrillic.to_lowercase();
            map.entry(key)
                .or_insert_with(Vec::new)
                .push(mongolian.to_string());
        }

        let mut dict = Self {
            map,
            entries: Vec::new(),
            cyrillic_index: Vec::new(),
            mongolian_index: Vec::new(),
            reverse_map: HashMap::new(),
        };
        dict.build_indices();
        dict
    }

    /// Strips Mongolian format-control characters (U+180B/C/D/E/F,
    /// U+202F) from `s` - used to normalize Mongolian text for
    /// indexing/matching, since two visually-identical words that differ
    /// only in which invisible variation selectors they contain should
    /// still match each other.
    fn normalize_mongolian(s: &str) -> String {
        s.chars().filter(|c| !is_format_control(*c)).collect()
    }

    /// Builds `entries`/`cyrillic_index`/`mongolian_index` from `map`.
    /// Called once by `from_tsv`/`load`; safe to call again (e.g. if a
    /// future phase supports reloading/merging dictionaries) since it
    /// fully rebuilds rather than appending.
    fn build_indices(&mut self) {
        let mut entries: Vec<(String, String)> = Vec::new();
        for (cyrillic, variants) in self.map.iter() {
            if cyrillic.chars().any(char::is_whitespace) {
                continue;
            }
            for mongolian in variants {
                if mongolian.chars().any(char::is_whitespace) {
                    continue;
                }
                entries.push((cyrillic.clone(), mongolian.clone()));
            }
        }

        let mut cyrillic_index: Vec<(String, usize)> = entries
            .iter()
            .enumerate()
            .map(|(i, (cyrillic, _))| (cyrillic.clone(), i))
            .collect();
        cyrillic_index.sort_by(|a, b| a.0.cmp(&b.0));

        let mut mongolian_index: Vec<(String, usize)> = entries
            .iter()
            .enumerate()
            .map(|(i, (_, mongolian))| (Self::normalize_mongolian(mongolian), i))
            .collect();
        mongolian_index.sort_by(|a, b| a.0.cmp(&b.0));

        let mut reverse_map: HashMap<String, Vec<String>> = HashMap::new();
        for (cyrillic, variants) in self.map.iter() {
            for mongolian in variants {
                if mongolian.chars().any(char::is_whitespace) {
                    continue;
                }
                let key = Self::normalize_mongolian(mongolian);
                if key.is_empty() {
                    continue;
                }
                let list = reverse_map.entry(key).or_default();
                if !list.contains(cyrillic) {
                    list.push(cyrillic.clone());
                }
            }
        }
        for list in reverse_map.values_mut() {
            list.sort_by(|a, b| {
                a.chars()
                    .count()
                    .cmp(&b.chars().count())
                    .then_with(|| a.cmp(b))
            });
        }

        self.entries = entries;
        self.cyrillic_index = cyrillic_index;
        self.mongolian_index = mongolian_index;
        self.reverse_map = reverse_map;
    }

    /// Cyrillic translations of a Mongolian Bichig word (the inverse of
    /// `lookup()`), for the double-click translation popup. Matching
    /// ignores Mongolian format-control characters on both sides, so a
    /// word typed with different FVS/MVS variants than the dictionary's
    /// still matches. Shorter Cyrillic forms rank first, then
    /// alphabetical; capped at `TRANSLATION_LIMIT`. Empty if unknown.
    pub fn reverse_lookup(&self, mongolian: &str) -> Vec<String> {
        let key = Self::normalize_mongolian(mongolian.trim());
        if key.is_empty() {
            return Vec::new();
        }
        self.reverse_map
            .get(&key)
            .map(|v| v.iter().take(TRANSLATION_LIMIT).cloned().collect())
            .unwrap_or_default()
    }

    /// Live word-completion/correction candidates for `prefix`, drawn
    /// from the single-word subset of the dictionary (see `entries`'
    /// doc comment). Always returns Mongolian Bichig strings - ready to
    /// insert into the document - regardless of which script `prefix`
    /// itself is in:
    ///
    /// * `Script::Cyrillic`: `prefix` is a partial Cyrillic word (the
    ///   "how do I write this in Bichig" case, brought inline from the
    ///   Transliteration modal's workflow) - matched against
    ///   `cyrillic_index`, returns each match's Mongolian value.
    /// * `Script::Mongolian`: `prefix` is a partial Mongolian Bichig word
    ///   already produced character-by-character via the Latin input
    ///   mode (the "predictive text completion" case) - matched against
    ///   `mongolian_index` (also returning Mongolian values - i.e. this
    ///   is Mongolian-to-Mongolian completion using the dictionary's
    ///   Bichig column as a word list).
    /// * `Script::Other`: no index exists for any other script; always
    ///   returns an empty list.
    ///
    /// Ranked: exact match (against the *query-side* key, before
    /// mapping to the returned Mongolian value) first, then shorter
    /// query-side keys (closer to what was actually typed), then
    /// alphabetical; deduplicated (distinct dictionary rows can share a
    /// Mongolian value); capped at `SUGGESTION_LIMIT`.
    pub fn suggest(&self, prefix: &str, script: Script) -> Vec<String> {
        let (index, key) = match script {
            Script::Cyrillic => (&self.cyrillic_index, prefix.trim().to_lowercase()),
            Script::Mongolian => (&self.mongolian_index, Self::normalize_mongolian(prefix.trim())),
            Script::Other => return Vec::new(),
        };
        if key.is_empty() {
            return Vec::new();
        }

        let start = index.partition_point(|(k, _)| k.as_str() < key.as_str());
        let mut candidates: Vec<(&str, usize)> = index[start..]
            .iter()
            .take_while(|(k, _)| k.starts_with(&key))
            .take(MAX_PREFIX_SCAN)
            .map(|(k, idx)| (k.as_str(), *idx))
            .collect();

        candidates.sort_by(|a, b| {
            let a_exact = a.0 == key;
            let b_exact = b.0 == key;
            b_exact
                .cmp(&a_exact)
                .then_with(|| a.0.len().cmp(&b.0.len()))
                .then_with(|| a.0.cmp(b.0))
        });

        let mut seen: HashSet<&str> = HashSet::new();
        let mut results = Vec::new();
        for (_, idx) in candidates {
            let suggestion = self.entries[idx].1.as_str();
            if seen.insert(suggestion) {
                results.push(suggestion.to_string());
                if results.len() >= SUGGESTION_LIMIT {
                    break;
                }
            }
        }
        results
    }

    /// `reverse_lookup()` plus a stem fallback for the double-click
    /// popup: Mongolian writes case/possessive suffixes after a NNBSP
    /// (U+202F), which `word_boundary` treats as part of the same word,
    /// so a double-clicked inflected word (`stem<NNBSP>suffix`) would
    /// never match a dictionary headword. If the whole word misses and
    /// contains a NNBSP, retry with the part before the first one.
    /// Returns `(candidates, used_stem)`.
    pub fn reverse_lookup_word(&self, word: &str) -> (Vec<String>, bool) {
        let whole = self.reverse_lookup(word);
        if !whole.is_empty() {
            return (whole, false);
        }
        if let Some((stem, _)) = word.split_once('\u{202F}') {
            let from_stem = self.reverse_lookup(stem);
            if !from_stem.is_empty() {
                return (from_stem, true);
            }
        }
        (Vec::new(), false)
    }

    /// Returns the list of Mongolian variants for a given Cyrillic input.
    /// The lookup is case-insensitive and trims surrounding whitespace.
    pub fn lookup(&self, cyrillic: &str) -> Option<&[String]> {
        let key = cyrillic.trim().to_lowercase();
        if key.is_empty() {
            return None;
        }
        self.map.get(&key).map(|v| v.as_slice())
    }

    /// Total number of unique Cyrillic keys loaded.
    pub fn len(&self) -> usize {
        self.map.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Dictionary {
        // Deliberately includes: a duplicate Cyrillic key with two
        // Mongolian variants (тест), a phrase entry on each side that
        // must be excluded from the suggestion indices, a Mongolian
        // value containing a format-control character (to exercise
        // normalization), and enough short-prefix entries to check
        // ranking.
        Dictionary::from_tsv(
            "# header comment\n\
             тест\t\u{1820}\u{1821}\n\
             тест\t\u{1820}\u{1822}\n\
             тестов\t\u{1820}\u{1821}\u{1823}\n\
             те\t\u{1820}\n\
             тест фраза\t\u{1820}\u{1821}\u{1824}\n\
             мон\t\u{1825}\u{180B}\u{1826}\n\
             монгол\t\u{1825}\u{1826}\u{1827}\n",
        )
    }

    #[test]
    fn lookup_is_unaffected_by_index_building_and_still_sees_phrases() {
        let dict = fixture();
        assert_eq!(dict.lookup("тест").map(|v| v.len()), Some(2));
        assert!(dict.lookup("тест фраза").is_some());
    }

    #[test]
    fn suggest_cyrillic_returns_mongolian_values_ranked_by_closeness() {
        let dict = fixture();
        let results = dict.suggest("тест", Script::Cyrillic);
        // "тест" itself is an exact match (2 variants -> 2 entries),
        // "тестов" is a longer prefix match - exact-match entries sort
        // first, and the phrase "тест фраза" must not appear at all.
        assert!(!results.is_empty());
        assert!(results.iter().all(|r| !r.contains(' ')));
        assert!(results.contains(&"\u{1820}\u{1821}".to_string()));
        assert!(results.contains(&"\u{1820}\u{1822}".to_string()));
    }

    #[test]
    fn suggest_cyrillic_excludes_phrase_entries() {
        let dict = fixture();
        let results = dict.suggest("тест фра", Script::Cyrillic);
        assert!(results.is_empty());
    }

    #[test]
    fn suggest_cyrillic_is_case_insensitive_and_prefix_based() {
        let dict = fixture();
        let results = dict.suggest("ТЕ", Script::Cyrillic);
        // Matches "те", "тест" (x2), "тестов" - not "тест фраза" (phrase).
        assert!(results.len() >= 3);
    }

    #[test]
    fn suggest_mongolian_completes_using_the_bichig_column_as_a_word_list() {
        let dict = fixture();
        // Typed so far (via Latin input mode): U+1825 (matches "мон"'s
        // and "монгол"'s Mongolian value prefix).
        let results = dict.suggest("\u{1825}", Script::Mongolian);
        assert!(results.contains(&"\u{1825}\u{1826}\u{1827}".to_string()));
    }

    #[test]
    fn suggest_mongolian_ignores_format_control_characters_when_matching() {
        let dict = fixture();
        // "мон" -> "\u{1825}\u{180B}\u{1826}" (format control embedded).
        // Searching by the same prefix *without* the format control
        // character must still find it - normalization must apply
        // symmetrically to both the index and the query.
        let results = dict.suggest("\u{1825}\u{1826}", Script::Mongolian);
        assert!(results.contains(&"\u{1825}\u{180B}\u{1826}".to_string()));
    }

    #[test]
    fn suggest_other_script_returns_empty() {
        let dict = fixture();
        assert!(dict.suggest("hello", Script::Other).is_empty());
    }

    #[test]
    fn suggest_empty_prefix_returns_empty() {
        let dict = fixture();
        assert!(dict.suggest("   ", Script::Cyrillic).is_empty());
    }

    #[test]
    fn suggest_deduplicates_identical_mongolian_values() {
        let dict = Dictionary::from_tsv(
            "дубль\t\u{1820}\u{1821}\n\
             дубльх\t\u{1820}\u{1821}\n",
        );
        let results = dict.suggest("дубль", Script::Cyrillic);
        let unique: HashSet<&String> = results.iter().collect();
        assert_eq!(results.len(), unique.len());
    }

    #[test]
    fn suggest_caps_results_at_the_suggestion_limit() {
        let mut tsv = String::new();
        for i in 0..(SUGGESTION_LIMIT + 10) {
            tsv.push_str(&format!("слово{i}\t\u{1820}{i}\n"));
        }
        let dict = Dictionary::from_tsv(&tsv);
        let results = dict.suggest("слово", Script::Cyrillic);
        assert_eq!(results.len(), SUGGESTION_LIMIT);
    }

    #[test]
    fn reverse_lookup_returns_cyrillic_for_a_bichig_word() {
        let dict = fixture();
        assert_eq!(dict.reverse_lookup("\u{1825}\u{1826}\u{1827}"), vec!["монгол"]);
    }

    #[test]
    fn reverse_lookup_ignores_format_control_characters_on_both_sides() {
        let dict = fixture();
        // Dictionary side has U+180B embedded; query side doesn't.
        assert_eq!(dict.reverse_lookup("\u{1825}\u{1826}"), vec!["мон"]);
        // And the other way round.
        assert_eq!(dict.reverse_lookup("\u{1825}\u{180C}\u{1826}"), vec!["мон"]);
    }

    #[test]
    fn reverse_lookup_returns_every_homograph_shortest_first() {
        let dict = Dictionary::from_tsv(
            "длинное\t\u{1820}\u{1821}\n\
             кор\t\u{1820}\u{1821}\n\
             аб\t\u{1820}\u{1821}\n\
             аб\t\u{1820}\u{1821}\n",
        );
        assert_eq!(dict.reverse_lookup("\u{1820}\u{1821}"), vec!["аб", "кор", "длинное"]);
    }

    #[test]
    fn reverse_lookup_keeps_cyrillic_phrases_but_skips_bichig_phrases() {
        let dict = Dictionary::from_tsv(
            "два слова\t\u{1820}\u{1821}\n\
             слово\t\u{1820} \u{1822}\n",
        );
        assert_eq!(dict.reverse_lookup("\u{1820}\u{1821}"), vec!["два слова"]);
        assert!(dict.reverse_lookup("\u{1820}").is_empty());
    }

    #[test]
    fn reverse_lookup_miss_and_blank_return_empty() {
        let dict = fixture();
        assert!(dict.reverse_lookup("\u{1830}\u{1831}").is_empty());
        assert!(dict.reverse_lookup("   ").is_empty());
        assert!(dict.reverse_lookup("\u{180B}").is_empty());
    }

    #[test]
    fn reverse_lookup_caps_at_the_translation_limit() {
        let mut tsv = String::new();
        for i in 0..(TRANSLATION_LIMIT + 5) {
            tsv.push_str(&format!("слово{i}\t\u{1820}\u{1821}\n"));
        }
        let dict = Dictionary::from_tsv(&tsv);
        assert_eq!(dict.reverse_lookup("\u{1820}\u{1821}").len(), TRANSLATION_LIMIT);
    }

    #[test]
    fn reverse_lookup_word_falls_back_to_the_stem_before_a_nnbsp_suffix() {
        let dict = fixture();
        let inflected = "\u{1825}\u{1826}\u{1827}\u{202F}\u{1828}\u{1829}";
        assert_eq!(dict.reverse_lookup_word(inflected), (vec!["монгол".to_string()], true));
        // Whole-word hit wins and doesn't report a stem fallback.
        assert_eq!(
            dict.reverse_lookup_word("\u{1825}\u{1826}\u{1827}"),
            (vec!["монгол".to_string()], false)
        );
        assert_eq!(dict.reverse_lookup_word("\u{1830}\u{202F}\u{1831}"), (Vec::new(), false));
    }
}

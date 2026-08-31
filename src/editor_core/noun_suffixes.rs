//! Data-driven noun-case suffix suggestions for Traditional Mongolian.
//!
//! Case suffixes are separated from a Bichig stem by U+202F (NNBSP), not
//! an ordinary space. The catalog deliberately stores fully authored Bichig
//! suffix strings: FVS/MVS decisions belong to the lexical spelling and must
//! not be guessed from a Latin transliteration.

use std::sync::OnceLock;

use crate::editor_core::format_control::is_format_control;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HarmonyClass {
    Masculine,
    Feminine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EndingCondition {
    Any,
    VowelOrDiphthong,
    Consonant,
    ConsonantNa,
    ConsonantOther,
    VowelOrSoftClosed,
    HardClosed,
}

#[derive(Debug)]
struct SuffixRule {
    case: String,
    ending: EndingCondition,
    harmony: Option<HarmonyClass>,
    rank: u8,
    form: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseSuffixSuggestion {
    pub case: String,
    pub form: String,
}

const CATALOG: &str = include_str!("noun_case_suffixes.tsv");

fn rules() -> &'static [SuffixRule] {
    static RULES: OnceLock<Vec<SuffixRule>> = OnceLock::new();
    RULES.get_or_init(|| {
        CATALOG
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .filter_map(parse_rule)
            .collect()
    })
}

fn parse_rule(line: &str) -> Option<SuffixRule> {
    let mut fields = line.split('\t');
    let case = fields.next()?.to_string();
    let ending = match fields.next()? {
        "any" => EndingCondition::Any,
        "vowel_or_diphthong" => EndingCondition::VowelOrDiphthong,
        "consonant" => EndingCondition::Consonant,
        "consonant_na" => EndingCondition::ConsonantNa,
        "consonant_other" => EndingCondition::ConsonantOther,
        "vowel_or_soft_closed" => EndingCondition::VowelOrSoftClosed,
        "hard_closed" => EndingCondition::HardClosed,
        _ => return None,
    };
    let harmony = match fields.next()? {
        "any" => None,
        "masculine" => Some(HarmonyClass::Masculine),
        "feminine" => Some(HarmonyClass::Feminine),
        _ => return None,
    };
    let rank = fields.next()?.parse().ok()?;
    let form = fields.next()?.to_string();
    Some(SuffixRule { case, ending, harmony, rank, form })
}

/// Classifies vowel harmony directly from the typed Bichig stem. The first
/// non-neutral vowel selects the class; a stem containing only neutral i is
/// feminine. This intentionally does not depend on a dictionary/Cyrillic
/// spelling: keyboard input can use a valid Unicode sequence that differs
/// from the dictionary's preferred glyph variant.
fn classify_harmony(stem: &str) -> Option<HarmonyClass> {
    let mut has_neutral_i = false;
    for c in stem.chars() {
        match c {
            '\u{1820}' | '\u{1823}' | '\u{1824}' => return Some(HarmonyClass::Masculine),
            '\u{1821}' | '\u{1825}' | '\u{1826}' => return Some(HarmonyClass::Feminine),
            '\u{1822}' => has_neutral_i = true,
            _ => {}
        }
    }
    has_neutral_i.then_some(HarmonyClass::Feminine)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StemEnding {
    VowelOrDiphthong,
    ConsonantNa,
    SoftClosed,
    HardClosed,
    ConsonantOther,
}

fn classify_ending(stem: &str) -> Option<StemEnding> {
    let final_letter = stem.chars().rev().find(|c| !is_format_control(*c))?;
    Some(match final_letter {
        // Vowels, plus ja (ᠶ) which closes a diphthong.
        '\u{1820}'..='\u{1826}' | '\u{1836}' => StemEnding::VowelOrDiphthong,
        '\u{1828}' => StemEnding::ConsonantNa,
        // ma, la, ang, wa. na and ya are included above where their
        // separate genitive/diphthong behavior takes precedence.
        '\u{182E}' | '\u{182F}' | '\u{1829}' | '\u{1838}' => StemEnding::SoftClosed,
        // ba, qa, ga, ra, sa, ta, da.
        '\u{182A}' | '\u{182C}' | '\u{182D}' | '\u{1837}' | '\u{1830}' | '\u{1832}'
        | '\u{1833}' => StemEnding::HardClosed,
        _ => StemEnding::ConsonantOther,
    })
}

fn ending_matches(condition: EndingCondition, ending: StemEnding) -> bool {
    match condition {
        EndingCondition::Any => true,
        EndingCondition::VowelOrDiphthong => ending == StemEnding::VowelOrDiphthong,
        EndingCondition::Consonant => ending != StemEnding::VowelOrDiphthong,
        EndingCondition::ConsonantNa => ending == StemEnding::ConsonantNa,
        EndingCondition::ConsonantOther => {
            matches!(ending, StemEnding::SoftClosed | StemEnding::HardClosed | StemEnding::ConsonantOther)
        }
        EndingCondition::VowelOrSoftClosed => {
            matches!(ending, StemEnding::VowelOrDiphthong | StemEnding::SoftClosed | StemEnding::ConsonantNa)
        }
        EndingCondition::HardClosed => ending == StemEnding::HardClosed,
    }
}

/// Returns every case suffix valid for the typed Bichig stem. The caller
/// inserts U+202F before `form` when accepting it.
pub fn suggest_case_suffixes(stem_bichig: &str) -> Vec<CaseSuffixSuggestion> {
    let Some(harmony) = classify_harmony(stem_bichig) else {
        return Vec::new();
    };
    let Some(ending) = classify_ending(stem_bichig) else {
        return Vec::new();
    };

    let mut matches: Vec<&SuffixRule> = rules()
        .iter()
        .filter(|rule| {
            ending_matches(rule.ending, ending)
                && rule.harmony.is_none_or(|required| required == harmony)
        })
        .collect();
    matches.sort_by(|a, b| a.rank.cmp(&b.rank).then_with(|| a.case.cmp(&b.case)));
    matches
        .into_iter()
        .map(|rule| CaseSuffixSuggestion { case: rule.case.clone(), form: rule.form.clone() })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vowel_harmony_uses_typed_bichig_vowels() {
        assert_eq!(classify_harmony("ᠠᠬ᠎ᠠ"), Some(HarmonyClass::Masculine));
        assert_eq!(classify_harmony("ᠡᠭᠴᠢ"), Some(HarmonyClass::Feminine));
        assert_eq!(classify_harmony("ᠴᠢᠷᠢᠭ"), Some(HarmonyClass::Feminine));
    }

    #[test]
    fn known_stems_receive_the_expected_case_forms() {
        let noun = "ᠨᠣᠮ";
        let forms: Vec<String> = suggest_case_suffixes(noun)
            .into_iter()
            .map(|suggestion| suggestion.form)
            .collect();
        assert!(forms.contains(&"ᠤᠨ".into()));
        assert!(forms.contains(&"ᠢ".into()));
        assert!(forms.contains(&"ᠳᠤ".into()));
        assert!(forms.contains(&"ᠳᠤᠷ".into()));
        assert!(forms.contains(&"ᠠᠴᠠ".into()));
        assert!(forms.contains(&"ᠲᠠᠢ".into()));
        assert!(forms.contains(&"ᠢᠶᠠᠷ".into()));
    }

    #[test]
    fn feminine_vowel_final_stem_uses_feminine_forms() {
        let forms: Vec<String> = suggest_case_suffixes("ᠡᠭᠴᠢ")
            .into_iter()
            .map(|suggestion| suggestion.form)
            .collect();
        assert!(forms.contains(&"ᠶᠢᠨ".into()));
        assert!(forms.contains(&"ᠶᠢ".into()));
        assert!(forms.contains(&"ᠳᠦ".into()));
        assert!(forms.contains(&"ᠡᠴᠡ".into()));
        assert!(forms.contains(&"ᠲᠡᠢ".into()));
        assert!(forms.contains(&"ᠪᠡᠷ".into()));
    }

    #[test]
    fn hard_closed_stems_take_t_forms_for_dative_locative() {
        let forms: Vec<String> = suggest_case_suffixes("ᠵᠢᠷᠤᠭ")
            .into_iter()
            .map(|suggestion| suggestion.form)
            .collect();
        assert!(forms.contains(&"ᠲᠤ".into()));
        assert!(forms.contains(&"ᠲᠤᠷ".into()));
        assert!(!forms.contains(&"ᠳᠤ".into()));
    }
}

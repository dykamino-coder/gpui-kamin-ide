//! Language-sensitive case mapping for `text-transform`.
//!
//! css-text-3 §2.1.1: «the UA must use the full case mappings for Unicode
//! characters, including any conditional casing rules, as defined in the
//! Unicode Standard» and «must also account for language-specific mappings
//! (SpecialCasing.txt)»; Greek uppercasing and the Dutch `IJ` digraph are
//! named there as tailorings UAs are expected to apply. Rust's
//! `to_uppercase`/`to_lowercase` implement only the language-independent
//! mappings, so the tailored languages are handled here.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tailoring {
    /// Turkish and Azeri: dotted/dotless i (SpecialCasing.txt `tr`, `az`).
    Turkic,
    /// Lithuanian: the dot above an `i` is kept explicit (SpecialCasing `lt`).
    Lithuanian,
    /// Greek uppercase drops accents (ICU `GreekUpper`, css-text-3 §2.1.1).
    Greek,
    /// Dutch: the `ij` digraph is titlecased together (css-text-3 §2.1.1).
    Dutch,
}

/// Tailoring for a BCP 47 tag; only the primary subtag counts, case-insensitively.
pub fn tailoring(lang: Option<&str>) -> Option<Tailoring> {
    let mut tags = lang?.split(['-', '_']);
    let primary = tags.next()?.to_ascii_lowercase();
    let (t, native) = match primary.as_str() {
        "tr" | "az" => (Tailoring::Turkic, "latn"),
        "lt" => (Tailoring::Lithuanian, "latn"),
        "el" => (Tailoring::Greek, "grek"),
        "nl" => (Tailoring::Dutch, "latn"),
        _ => return None,
    };
    // The tailoring belongs to the writing system, not to the language alone
    // (css-text-3 §1.4: `tr-Cyrl` is Turkish in Cyrillic, which has no
    // dotted/dotless i rule — `writing-system-text-transform-001`).
    let script = tags.find(|s| s.len() == 4 && s.chars().all(|c| c.is_ascii_alphabetic()));
    match script {
        Some(s) if !s.eq_ignore_ascii_case(native) => None,
        _ => Some(t),
    }
}

/// Combining marks with canonical combining class 230 (above) that matter
/// for the Lithuanian `More_Above` / `After_Soft_Dotted` conditions.
fn combining_above(ch: char) -> bool {
    matches!(ch as u32, 0x0300..=0x0314 | 0x033D..=0x0344 | 0x0346 | 0x034A..=0x034C | 0x0350..=0x0352 | 0x0357 | 0x035B | 0x0363..=0x036F)
}

fn combining(ch: char) -> bool {
    matches!(ch as u32, 0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F)
}

fn soft_dotted(ch: char) -> bool {
    matches!(
        ch,
        'i' | 'j'
            | '\u{12F}'
            | '\u{249}'
            | '\u{268}'
            | '\u{29D}'
            | '\u{2B2}'
            | '\u{3F3}'
            | '\u{456}'
            | '\u{458}'
            | '\u{1D62}'
            | '\u{1D96}'
            | '\u{1DA4}'
            | '\u{1DA8}'
            | '\u{1E2D}'
            | '\u{1ECB}'
            | '\u{2071}'
            | '\u{2148}'
            | '\u{2149}'
            | '\u{2C7C}'
    )
}

pub fn upper(text: &str, t: Tailoring) -> String {
    match t {
        Tailoring::Turkic => text
            .chars()
            .flat_map(|ch| -> Box<dyn Iterator<Item = char>> {
                if ch == 'i' {
                    Box::new(std::iter::once('\u{130}'))
                } else {
                    Box::new(ch.to_uppercase())
                }
            })
            .collect(),
        // `0307; 0307; ; ; lt After_Soft_Dotted; # COMBINING DOT ABOVE` —
        // in uppercase the explicit dot after a soft-dotted letter goes away.
        Tailoring::Lithuanian => {
            let mut out = String::with_capacity(text.len());
            let mut after_soft_dotted = false;
            for ch in text.chars() {
                if ch == '\u{307}' && after_soft_dotted {
                    continue;
                }
                if !combining(ch) {
                    after_soft_dotted = soft_dotted(ch);
                } else if combining_above(ch) {
                    after_soft_dotted = false;
                }
                out.extend(ch.to_uppercase());
            }
            out
        }
        Tailoring::Greek => greek_upper(text),
        Tailoring::Dutch => text.to_uppercase(),
    }
}

pub fn lower(text: &str, t: Tailoring) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    match t {
        Tailoring::Turkic => {
            let mut i = 0;
            while i < chars.len() {
                let ch = chars[i];
                match ch {
                    '\u{130}' => out.push('i'),
                    // `0307; ; 0307; 0307; tr After_I;` and `0049; 0131; …; tr Not_Before_Dot;`
                    'I' => {
                        let mut j = i + 1;
                        while j < chars.len() && combining(chars[j]) && !combining_above(chars[j]) {
                            j += 1;
                        }
                        if chars.get(j) == Some(&'\u{307}') {
                            out.push('i');
                            out.extend(&chars[i + 1..j]);
                            i = j + 1;
                            continue;
                        }
                        out.push('\u{131}');
                    }
                    _ => out.extend(ch.to_lowercase()),
                }
                i += 1;
            }
        }
        Tailoring::Lithuanian => {
            for (i, &ch) in chars.iter().enumerate() {
                let more_above = || {
                    chars[i + 1..]
                        .iter()
                        .take_while(|c| combining(**c))
                        .any(|c| combining_above(*c))
                };
                match ch {
                    'I' if more_above() => out.push_str("i\u{307}"),
                    'J' if more_above() => out.push_str("j\u{307}"),
                    '\u{12E}' if more_above() => out.push_str("\u{12F}\u{307}"),
                    '\u{CC}' => out.push_str("i\u{307}\u{300}"),
                    '\u{CD}' => out.push_str("i\u{307}\u{301}"),
                    '\u{128}' => out.push_str("i\u{307}\u{303}"),
                    _ => out.extend(ch.to_lowercase()),
                }
            }
        }
        Tailoring::Greek | Tailoring::Dutch => return text.to_lowercase(),
    }
    out
}

/// Titlecase of the first letter of a word; `rest` is the remainder of the
/// word, so the Dutch digraph can take its `j` along. Returns how many chars
/// of `rest` were consumed.
pub fn title_start(ch: char, rest: &[char], t: Tailoring, out: &mut String) -> Option<usize> {
    match (t, ch) {
        (Tailoring::Turkic, 'i') => {
            out.push('\u{130}');
            Some(0)
        }
        (Tailoring::Dutch, 'i' | 'I') if matches!(rest.first(), Some('j' | 'J')) => {
            out.push_str("IJ");
            Some(1)
        }
        _ => None,
    }
}

/// Greek letter → (uppercase base, has dialytika, had an accent).
fn greek_parts(ch: char) -> Option<(char, bool, bool)> {
    Some(match ch {
        'ά' | 'Ά' => ('Α', false, true),
        'έ' | 'Έ' => ('Ε', false, true),
        'ή' | 'Ή' => ('Η', false, true),
        'ί' | 'Ί' => ('Ι', false, true),
        'ό' | 'Ό' => ('Ο', false, true),
        'ύ' | 'Ύ' => ('Υ', false, true),
        'ώ' | 'Ώ' => ('Ω', false, true),
        'ΐ' => ('Ι', true, true),
        'ΰ' => ('Υ', true, true),
        'ϊ' | 'Ϊ' => ('Ι', true, false),
        'ϋ' | 'Ϋ' => ('Υ', true, false),
        _ => return None,
    })
}

fn greek_letter(ch: char) -> bool {
    matches!(ch as u32, 0x0370..=0x03FF | 0x1F00..=0x1FFF)
}

/// Greek uppercase without accents (ICU `GreekUpper::toUpper`): tonos,
/// varia and perispomeni are dropped, dialytika stays, an accented vowel
/// followed by `ι`/`υ` gives that letter a dialytika (`Νεράιδα` → `ΝΕΡΑΪΔΑ`),
/// and the disjunctive eta `ή` standing alone keeps its accent (`Ή`).
fn greek_upper(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut add_dialytika = false;
    for (i, &ch) in chars.iter().enumerate() {
        if matches!(ch, '\u{300}' | '\u{301}' | '\u{342}' | '\u{344}')
            && i > 0
            && greek_letter(chars[i - 1])
        {
            if ch == '\u{344}' {
                out.push('\u{308}');
            }
            continue;
        }
        let parts = greek_parts(ch);
        let (base, mut dia, accent) = match parts {
            Some(p) => p,
            None => {
                let up: String = ch.to_uppercase().collect();
                let mut it = up.chars();
                match (it.next(), it.next()) {
                    (Some(u), None) if matches!(u, 'Ι' | 'Υ') => (u, false, false),
                    _ => {
                        out.push_str(&up);
                        add_dialytika = false;
                        continue;
                    }
                }
            }
        };
        if add_dialytika && matches!(base, 'Ι' | 'Υ') && !accent {
            dia = true;
        }
        let alone = |j: Option<&char>| j.is_none_or(|c| !c.is_alphabetic());
        let disjunctive_eta = matches!(ch, 'ή')
            && alone(i.checked_sub(1).and_then(|j| chars.get(j)))
            && alone(chars.get(i + 1));
        if disjunctive_eta {
            out.push('Ή');
        } else {
            out.push(match (base, dia) {
                ('Ι', true) => 'Ϊ',
                ('Υ', true) => 'Ϋ',
                (b, _) => b,
            });
        }
        add_dialytika = accent && !dia && !matches!(base, 'Ι' | 'Υ');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turkic_and_lithuanian_follow_special_casing() {
        assert_eq!(upper("i ı", Tailoring::Turkic), "İ I");
        assert_eq!(lower("İ I\u{307} I", Tailoring::Turkic), "i i ı");
        assert_eq!(
            lower("Ì Í Ĩ", Tailoring::Lithuanian),
            "i\u{307}\u{300} i\u{307}\u{301} i\u{307}\u{303}"
        );
        assert_eq!(upper("i\u{307}\u{300}", Tailoring::Lithuanian), "I\u{300}");
    }

    #[test]
    fn greek_uppercase_drops_accents() {
        assert_eq!(greek_upper("καλημέρα αύριο"), "ΚΑΛΗΜΕΡΑ ΑΥΡΙΟ");
        assert_eq!(greek_upper("θεϊκό"), "ΘΕΪΚΟ");
        assert_eq!(greek_upper("ευφυΐα Νεράιδα"), "ΕΥΦΥΪΑ ΝΕΡΑΪΔΑ");
        assert_eq!(greek_upper("ήσουν ή εγώ ή εσύ"), "ΗΣΟΥΝ Ή ΕΓΩ Ή ΕΣΥ");
    }

    #[test]
    fn dutch_ij_and_tags() {
        let mut s = String::new();
        assert_eq!(
            title_start('i', &['j', 's'], Tailoring::Dutch, &mut s),
            Some(1)
        );
        assert_eq!(s, "IJ");
        assert_eq!(tailoring(Some("NL")), Some(Tailoring::Dutch));
        assert_eq!(tailoring(Some("tr-TR")), Some(Tailoring::Turkic));
        assert_eq!(tailoring(Some("en")), None);
        assert_eq!(tailoring(Some("tr-Cyrl")), None);
        assert_eq!(tailoring(Some("az-Latn-AZ")), Some(Tailoring::Turkic));
    }
}

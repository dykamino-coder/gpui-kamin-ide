//! Greek for lang_case; split out to keep the owning module within 250 lines.

/// Greek letter → (uppercase base, has dialytika, had an accent).
pub(super) fn greek_parts(ch: char) -> Option<(char, bool, bool)> {
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

pub(super) fn greek_letter(ch: char) -> bool {
    matches!(ch as u32, 0x0370..=0x03FF | 0x1F00..=0x1FFF)
}

/// Greek uppercase without accents (ICU `GreekUpper::toUpper`): tonos,
/// varia and perispomeni are dropped, dialytika stays, an accented vowel
/// followed by `ι`/`υ` gives that letter a dialytika (`Νεράιδα` → `ΝΕΡΑΪΔΑ`),
/// and the disjunctive eta `ή` standing alone keeps its accent (`Ή`).
pub(super) fn greek_upper(text: &str) -> String {
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

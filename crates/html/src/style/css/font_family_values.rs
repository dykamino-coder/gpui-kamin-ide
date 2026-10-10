//! Preserve identifier-token validity when decoding escaped font-family names.

pub(super) fn normalize(property: &str, value: &str) -> String {
    if property != "font-family" || !value.contains('\\') {
        return super::unescape_value(value);
    }
    serialize(value).unwrap_or_else(|| super::unescape_value(value))
}

fn serialize(value: &str) -> Option<String> {
    let end = super::top_level_bang(value).unwrap_or(value.len());
    let mut families = Vec::new();
    for part in super::split_top_level(&value[..end], ',') {
        let part = part.trim();
        if part.starts_with(['\'', '"']) || !part.contains('\\') {
            families.push(part.to_string());
            continue;
        }
        let names = identifiers(part)?;
        let name = names
            .iter()
            .map(|raw| super::unescape(raw))
            .collect::<Vec<_>>()
            .join(" ");
        // CSS Fonts 4 section 4.1.2: an identifier sequence and a string
        // denote the same family. Validate tokens before decoding (CSS
        // Syntax 3 sections 4.3.9/4.3.11); an escaped digit, punctuation or
        // control character remains valid inside that identifier token.
        // Preserve keyword tokens so escaped generic and CSS-wide names
        // retain their keyword meaning rather than becoming quoted families.
        let keyword = names.len() == 1
            && matches!(
                name.to_ascii_lowercase().as_str(),
                "inherit"
                    | "initial"
                    | "unset"
                    | "revert"
                    | "revert-layer"
                    | "serif"
                    | "sans-serif"
                    | "monospace"
                    | "cursive"
                    | "fantasy"
                    | "system-ui"
                    | "ui-serif"
                    | "ui-sans-serif"
                    | "ui-monospace"
                    | "ui-rounded"
                    | "emoji"
                    | "math"
                    | "fangsong"
            );
        families.push(if keyword { name } else { string(&name) });
    }
    let mut result = families.join(", ");
    if end < value.len() {
        result.push(' ');
        result.push_str(&value[end..]);
    }
    Some(result)
}

fn identifiers(raw: &str) -> Option<Vec<&str>> {
    let mut words = Vec::new();
    let mut at = 0;
    while at < raw.len() {
        while raw[at..].starts_with([' ', '\t', '\n', '\r', '\u{c}']) {
            at += 1;
        }
        if at == raw.len() {
            break;
        }
        let start = at;
        while at < raw.len() {
            let ch = raw[at..].chars().next()?;
            if ch == '\\' {
                at = super::variable_tokens::escape_end(raw, at);
            } else if matches!(ch, ' ' | '\t' | '\n' | '\r' | '\u{c}') {
                break;
            } else {
                at += ch.len_utf8();
            }
        }
        let word = &raw[start..at];
        if !super::selector_tokens::ident(word) {
            return None;
        }
        words.push(word);
    }
    (!words.is_empty()).then_some(words)
}

fn string(name: &str) -> String {
    let mut result = String::from("\"");
    for ch in name.chars() {
        match ch {
            '"' | '\\' => {
                result.push('\\');
                result.push(ch);
            }
            ch if ch.is_control() => result.push_str(&format!("\\{:X} ", ch as u32)),
            ch => result.push(ch),
        }
    }
    result.push('"');
    result
}

#[cfg(test)]
mod tests {
    use super::normalize;

    #[test]
    fn escaped_name_characters_remain_valid_family_strings() {
        assert_eq!(
            normalize("font-family", r"test\foo, Ahem"),
            r#""test\F oo", Ahem"#
        );
        assert_eq!(
            normalize("font-family", r"\31 foo, Ahem"),
            r#""1foo", Ahem"#
        );
        assert_eq!(
            normalize("font-family", r"test\+foo !important"),
            r#""test+foo" !important"#
        );
    }

    #[test]
    fn escaped_keywords_and_other_properties_keep_their_meaning() {
        assert_eq!(normalize("font-family", r"\69 nherit"), "inherit");
        assert_eq!(normalize("font-family", r"\6d onospace"), "monospace");
        assert_eq!(normalize("color", r"\67 reen"), "green");
    }
}

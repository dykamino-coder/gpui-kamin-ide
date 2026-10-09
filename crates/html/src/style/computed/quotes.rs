//! Keep automatic quotation systems distinct from inherited and suppressed ones.
//! CSS Content 3 §quotes-property: explicit auto overrides any inherited system.

use super::{Computed, unescape_content};

pub(super) fn apply(style: &mut Computed, v: &str) {
    match v.to_ascii_lowercase().as_str() {
        "none" => style.quotes = Some(None),
        // Empty pairs distinguish explicit auto from inheritance.
        "auto" => style.quotes = Some(Some(vec![])),
        "inherit" | "match-parent" => style.quotes = None,
        _ => {
            let other = v;
            // Только строки, и чётным числом (§4.1): иначе
            // объявление негодно и не применяется.
            let mut strs = vec![];
            let mut at = 0usize;
            let mut ok = true;
            while at < other.len() {
                let ch = other[at..].chars().next().unwrap_or(' ');
                if ch.is_whitespace() {
                    at += ch.len_utf8();
                    continue;
                }
                if ch != '"' && ch != '\'' {
                    ok = false;
                    break;
                }
                let body = at + 1;
                let len = crate::css::skip_string(&other[body..], ch);
                if !other[body..body + len].ends_with(ch) {
                    ok = false;
                    break;
                }
                strs.push(unescape_content(&other[body..body + len - 1]));
                at = body + len;
            }
            if ok && !strs.is_empty() && strs.len() % 2 == 0 {
                style.quotes = Some(Some(
                    strs.chunks(2)
                        .map(|p| (p[0].clone(), p[1].clone()))
                        .collect(),
                ));
            }
        }
    }
}

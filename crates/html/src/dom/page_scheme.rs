//! `<meta name=color-scheme>`: выбор схемы документа до разбора `@media`.
//!
//! Вход — дерево html5ever и предпочтение окружения; выход — тёмная ли схема
//! по первому верному `<meta>` в порядке дерева. `content` разбирается как
//! список CSS-идентификаторов с экранированием и комментариями: `only`,
//! `light`/`dark`, `normal` в одиночку; CSS-wide слова и мусор делают запись
//! негодной. Результат ставится в `Media::dark` перед каскадом.

use markup5ever_rcdom::{Handle, NodeData};

pub(super) fn preference(node: &Handle, preferred: bool) -> Option<bool> {
    if let NodeData::Element { name, attrs, .. } = &node.data
        && &*name.local == "meta"
        && &*name.ns == "http://www.w3.org/1999/xhtml"
    {
        let attrs = attrs.borrow();
        let attr = |key: &str| attrs.iter().find(|a| &*a.name.local == key);
        if attr("name").is_some_and(|a| a.value.eq_ignore_ascii_case("color-scheme"))
            && let Some(content) = attr("content")
            && let Some(dark) = supported(&content.value, preferred)
        {
            return Some(dark);
        }
    }
    node.children
        .borrow()
        .iter()
        .find_map(|n| preference(n, preferred))
}

fn supported(value: &str, preferred: bool) -> Option<bool> {
    let words = identifiers(value)?;
    let (mut light, mut dark, mut only, mut count) = (false, false, false, 0);
    for ident in &words {
        match ident.as_str() {
            "normal" if words.len() == 1 => return Some(preferred),
            "light" => light = true,
            "dark" => dark = true,
            "only" if !only => {
                only = true;
                continue;
            }
            "only" | "normal" | "initial" | "inherit" | "unset" | "revert" | "revert-layer"
            | "default" => return None,
            _ => {}
        }
        count += 1;
    }
    (count > 0).then_some(dark && (!light || preferred))
}

// CSS identifier tokens; escapes retain their optional terminating whitespace.
fn identifiers(mut value: &str) -> Option<Vec<String>> {
    let mut words = Vec::new();
    while !value.is_empty() {
        value = value.trim_start_matches([' ', '\t', '\n', '\r', '\u{c}']);
        if value.is_empty() {
            break;
        }
        if let Some(rest) = value.strip_prefix("/*") {
            value = rest.find("*/").map_or("", |end| &rest[end + 2..]);
            continue;
        }
        let mut end = 0;
        while end < value.len() {
            let ch = value[end..].chars().next()?;
            if ch.is_ascii_whitespace() || value[end..].starts_with("/*") {
                break;
            }
            end += ch.len_utf8();
            if ch == '\\' {
                let escaped = value[end..].chars().next()?;
                if escaped.is_ascii_hexdigit() {
                    for _ in 0..6 {
                        let Some(digit) = value[end..].chars().next() else {
                            break;
                        };
                        if !digit.is_ascii_hexdigit() {
                            break;
                        }
                        end += digit.len_utf8();
                    }
                    if let Some(space) = value[end..]
                        .chars()
                        .next()
                        .filter(|c| c.is_ascii_whitespace())
                    {
                        end += space.len_utf8();
                        if space == '\r' && value[end..].starts_with('\n') {
                            end += 1;
                        }
                    }
                } else {
                    end += escaped.len_utf8();
                }
            }
        }
        let raw = &value[..end];
        if !crate::style::css::selector_tokens::ident(raw) {
            return None;
        }
        words.push(crate::style::css::unescape(raw).to_ascii_lowercase());
        value = &value[end..];
    }
    Some(words)
}

#[cfg(test)]
mod tests {
    use super::*;
    use html5ever::tendril::TendrilSink;

    #[test]
    fn css_identifiers_and_supported_scheme_selection() {
        assert_eq!(supported("only dark", false), Some(true));
        assert_eq!(supported("light dark", true), Some(true));
        assert_eq!(supported("light dark", false), Some(false));
        assert_eq!(supported("d\\61 rk/**/only", false), Some(true));
        for invalid in ["", "only", "dark normal", "dark,light", "only dark only"] {
            assert_eq!(supported(invalid, false), None, "{invalid}");
        }
    }

    #[test]
    fn first_valid_metadata_in_tree_order_wins() {
        let dom = html5ever::parse_document(markup5ever_rcdom::RcDom::default(), Default::default()).one(
            "<meta name=color-scheme content='only'><meta name=COLOR-SCHEME content=dark><meta name=color-scheme content=light>"
        );
        assert_eq!(preference(&dom.document, false), Some(true));
    }
}

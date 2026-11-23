//! Сравнение значений разных документов с согласованным переименованием ключей.

use super::Document;

/// Дерево и стиль сравниваются по значениям. Owner и ключи отдельных DOM
/// нормализуются; повторные ссылки на один ключ сохраняют связь, а synthetic — 0.
pub(super) fn tree(doc: &Document) -> String {
    let text = format!("{:?}\n{:?}", doc.nodes(), doc.root_style());
    normalize(&text)
}

fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    let mut keys = std::collections::HashMap::new();
    while !rest.is_empty() {
        // Debug strings can themselves contain field names, escaped quotes, and
        // #digits. Preserve their payload before considering a TextId suffix.
        let field = if rest.starts_with('"') {
            let mut escaped = false;
            let end = rest[1..]
                .char_indices()
                .find_map(|(i, c)| {
                    if escaped {
                        escaped = false;
                    } else if c == '\\' {
                        escaped = true;
                    } else if c == '"' {
                        return Some(i + 2);
                    }
                    None
                })
                .expect("closed Debug string");
            out.push_str(&rest[..end]);
            rest = &rest[end..];
            rest.starts_with('#').then_some("#")
        } else {
            [
                "owner: ",
                "node_id: ",
                "decor_box: ",
                "implicit_anchor: Some(",
            ]
            .into_iter()
            .find(|field| rest.starts_with(field))
        };
        let Some(field) = field else {
            if let Some(c) = rest.chars().next() {
                out.push(c);
                rest = &rest[c.len_utf8()..];
            }
            continue;
        };
        out.push_str(field);
        rest = &rest[field.len()..];
        let end = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        if field == "owner: " {
            out.push('_');
        } else if let Ok(key) = rest[..end].parse::<u64>() {
            let next = keys.len() + 1;
            let normalized = if key == 0 {
                0
            } else {
                *keys.entry(key).or_insert(next)
            };
            out.push_str(&normalized.to_string());
        }
        rest = &rest[end..];
    }
    out
}

#[test]
fn identities_are_renamed_consistently_without_editing_text_payloads() {
    let a = r###"node_id: 55 Text("node_id: 12 \"#99"#55) decor_box: 66 Text("x"#66)"###;
    let b = r###"node_id: 2 Text("node_id: 12 \"#99"#2) decor_box: 3 Text("x"#3)"###;
    assert_eq!(normalize(a), normalize(b));
    assert_ne!(normalize(a), normalize(&b.replace("#3)", "#2)")));
    assert_ne!(normalize(a), normalize(&b.replace("#99", "#98")));
}

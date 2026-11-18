//! Сравнение значений разных документов с согласованным переименованием ключей.

use super::Document;

/// Дерево и стиль сравниваются по значениям. Owner и ключи отдельных DOM
/// нормализуются; повторные ссылки на один ключ сохраняют связь, а synthetic — 0.
pub(super) fn tree(doc: &Document) -> String {
    let text = value(&format!("{:?}\n{:?}", doc.nodes(), doc.root_style()));
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();
    let mut keys = std::collections::HashMap::new();
    while let Some((i, field)) = [
        "owner: ",
        "node_id: ",
        "decor_box: ",
        "implicit_anchor: Some(",
    ]
    .into_iter()
    .filter_map(|field| rest.find(field).map(|i| (i, field)))
    .min_by_key(|(i, _)| *i)
    {
        out.push_str(&rest[..i + field.len()]);
        rest = &rest[i + field.len()..];
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
    out.push_str(rest);
    out
}

/// Exclude a mutable diagnostic journal from comparisons of CSS/DOM values.
/// Payload sizes and all other fields remain part of the comparison.
pub(super) fn value(text: &str) -> String {
    const OWNER: &str = "Owner(Cell { value: Snapshot { ";
    const END: &str = " } })";
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(['"', 'O']) {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        if rest.starts_with('"') {
            let mut escaped = false;
            let end = rest[1..]
                .char_indices()
                .find_map(|(i, ch)| {
                    if escaped {
                        escaped = false;
                    } else if ch == '\\' {
                        escaped = true;
                    } else if ch == '"' {
                        return Some(i + 2);
                    }
                    None
                })
                .unwrap_or(rest.len());
            out.push_str(&rest[..end]);
            rest = &rest[end..];
        } else if let Some(body) = rest.strip_prefix(OWNER)
            && let Some(end) = body.find(END)
            && storage_snapshot(&body[..end])
        {
            out.push_str("Owner(_)");
            rest = &body[end + END.len()..];
        } else {
            out.push('O');
            rest = &rest[1..];
        }
    }
    out.push_str(rest);
    out
}

fn storage_snapshot(text: &str) -> bool {
    let mut fields = text.split(", ");
    [
        "computed_headers",
        "inherited_headers",
        "group_allocations",
        "group_payload_bytes",
    ]
    .into_iter()
    .all(|name| {
        fields.next().is_some_and(|field| {
            field
                .strip_prefix(name)
                .and_then(|v| v.strip_prefix(": "))
                .is_some_and(|v| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()))
        })
    }) && fields.next().is_none()
}

#[cfg(test)]
mod value_tests {
    use super::value;

    fn owner(count: u64) -> String {
        format!(
            "Owner(Cell {{ value: Snapshot {{ computed_headers: {count}, inherited_headers: 2, group_allocations: 3, group_payload_bytes: 4 }} }})"
        )
    }

    #[test]
    fn storage_history_is_not_a_css_value() {
        let input = |count, bytes| {
            format!(
                "CloneAudit {{ owner: Some({}) }}, Allocation(Some(({}, {bytes}))), width: Px(7.0)",
                owner(count),
                owner(count)
            )
        };
        assert_eq!(value(&input(1, 64)), value(&input(9, 64)));
        assert_ne!(value(&input(1, 64)), value(&input(1, 128)));
        assert_ne!(
            value(&input(1, 64)),
            value(&input(1, 64).replace("Px(7.0)", "Px(8.0)"))
        );
    }

    #[test]
    fn authored_text_and_unknown_owner_shapes_remain_visible() {
        let authored = format!(
            "text: {:?}, actual: {}",
            format!("\"{}\"", owner(1)),
            owner(2)
        );
        let normalized = value(&authored);
        assert!(normalized.contains(&format!("{:?}", format!("\"{}\"", owner(1)))));
        assert!(normalized.ends_with("actual: Owner(_)"));
        let unknown = owner(1).replace("computed_headers: 1", "future_field: 1");
        assert_eq!(value(&unknown), unknown);
    }
}

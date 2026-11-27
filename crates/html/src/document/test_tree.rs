//! Сравнение значений разных документов с согласованным переименованием ключей.

use super::Document;

/// Compare tree and style values, normalizing document-local identities.
/// Repeated node references retain their relationship.
pub(super) fn tree(doc: &Document) -> String {
    let text = format!("{:?}\n{:?}", doc.nodes(), doc.root_style());
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();
    let mut keys = std::collections::HashMap::new();
    // Rebuilt subtrees can allocate text and synthetic paint identities
    // in a different order than a fresh build.
    let mut texts = std::collections::HashMap::new();
    while let Some((i, field)) = [
        "owner: ",
        "node_id: ",
        "decor_box: ",
        "implicit_anchor: Some(",
        "\"#",
        "state_id: OnceCell(",
    ]
    .into_iter()
    .filter_map(|field| rest.find(field).map(|i| (i, field)))
    .min_by_key(|(i, _)| *i)
    {
        out.push_str(&rest[..i + field.len()]);
        rest = &rest[i + field.len()..];
        // Synthetic paint identities are allocated lazily during an edit;
        // an untouched fresh build can still have an empty cell.
        if field.starts_with("state_id") {
            let end = rest.find(')').expect("a debug OnceCell ends with ')'");
            out.push('_');
            rest = &rest[end..];
            continue;
        }
        let end = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        if field == "owner: " {
            out.push('_');
        } else if field == "\"#" {
            let next = texts.len() + 1;
            out.push_str(
                &texts
                    .entry(rest[..end].to_string())
                    .or_insert(next)
                    .to_string(),
            );
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

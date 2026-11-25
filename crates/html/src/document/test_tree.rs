//! Сравнение значений разных документов с согласованным переименованием ключей.

use super::Document;

/// Дерево и стиль сравниваются по значениям. Owner и ключи отдельных DOM
/// нормализуются; повторные ссылки на один ключ сохраняют связь, а synthetic — 0.
pub(super) fn tree(doc: &Document) -> String {
    let text = format!("{:?}\n{:?}", doc.nodes(), doc.root_style());
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();
    let mut keys = std::collections::HashMap::new();
    // Source text numbers (`TextId`, `"text"#n`) and the state identities of
    // synthetic boxes are identities as node ids are: a rebuilt subtree
    // numbers a text the build did not reach, and a new synthetic box, next.
    let mut texts = std::collections::HashMap::new();
    let mut synthetic = std::collections::HashMap::new();
    while let Some((i, field)) = [
        "owner: ",
        "node_id: ",
        "decor_box: ",
        "implicit_anchor: Some(",
        "\"#",
        "state_id: OnceCell(\"",
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
        } else if field == "\"#" {
            let next = texts.len() + 1;
            out.push_str(
                &texts
                    .entry(rest[..end].to_string())
                    .or_insert(next)
                    .to_string(),
            );
        } else if field.starts_with("state_id") {
            let next = synthetic.len();
            out.push_str(
                &synthetic
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

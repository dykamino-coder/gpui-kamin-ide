//! `@custom-media` (mediaqueries-5 §5.1): имена-псевдонимы запросов, собранные из таблицы и
//! подставляемые в `(--имя)`.

use super::*;
use crate::values::ValueCx;
use std::collections::HashMap;

/// Псевдонимы таблицы `css` — `@custom-media --имя <запрос>;` верхнего
/// уровня: имя (в нижнем регистре, как и весь запрос) → список запросов либо
/// `true`/`false`. Последнее определение имени побеждает (§5.1).
pub fn custom_media_of(css: &str) -> HashMap<String, String> {
    let low = strip_comments(css).to_ascii_lowercase();
    let mut map = HashMap::new();
    let mut rest = low.as_str();
    while let Some(at) = rest.find("@custom-media") {
        let tail = &rest[at + "@custom-media".len()..];
        let end = tail.find(';').unwrap_or(tail.len());
        let body = tail[..end].trim();
        if let Some((name, query)) = body.split_once(char::is_whitespace)
            && name.starts_with("--")
        {
            map.insert(name.to_string(), query.trim().to_string());
        }
        rest = &tail[end..];
    }
    map
}

/// Неопределённый alias остаётся unknown, включая отрицание (MQ §3.2).
pub(super) fn eval_custom(
    _media: &Media,
    name: &str,
    custom: CustomMedia,
    _vcx: ValueCx<'_>,
) -> SupTri {
    custom
        .aliases
        .and_then(|a| a.get(name))
        .copied()
        .unwrap_or(SupTri::Unknown)
}

mod graph;
pub use graph::resolve_custom_media;

#[cfg(test)]
mod tests;

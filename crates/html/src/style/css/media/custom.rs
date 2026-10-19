//! `@custom-media` (mediaqueries-5 §5.1): имена-псевдонимы запросов, собранные из таблицы и подставляемые в `(--имя)`.

use super::*;
use crate::document::state::ValueCx;
use std::collections::HashMap;

/// Псевдонимы таблицы `css` — `@custom-media --имя <запрос>;` верхнего
/// уровня: имя (в нижнем регистре, как и весь запрос) → список запросов либо
/// `true`/`false`. Последнее определение имени побеждает (§5.1).
pub(crate) fn custom_media_of(css: &str) -> HashMap<String, String> {
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

/// Значение `(--имя)`: неизвестное имя — ложь (§5.1 «evaluates to false»).
/// Глубина подстановки ограничена: циклическая ссылка не уходит в
/// бесконечность (§5.1 «cyclic … are invalid» — результатом служит ложь).
pub(super) fn eval_custom(
    media: &Media,
    name: &str,
    custom: CustomMedia,
    vcx: ValueCx<'_>,
) -> SupTri {
    let Some(query) = custom.aliases.and_then(|a| a.get(name)) else {
        return SupTri::False;
    };
    let v = match query.as_str() {
        "true" => true,
        "false" => false,
        _ if custom.depth < 8 => media.matches_in(
            query,
            CustomMedia {
                depth: custom.depth + 1,
                ..custom
            },
            vcx,
        ),
        _ => false,
    };
    if v { SupTri::True } else { SupTri::False }
}

//! `@custom-media` (mediaqueries-5 §5.1): имена-псевдонимы запросов, собранные из таблицы и подставляемые в `(--имя)`.

use super::*;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

thread_local! {
    /// Псевдонимы ТЕКУЩЕЙ разбираемой таблицы: имя (в нижнем регистре,
    /// как и весь запрос) → список запросов либо `true`/`false`.
    static CUSTOM: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
    /// Глубина подстановки: циклическая ссылка не уходит в бесконечность
    /// (§5.1 «cyclic … are invalid» — результатом служит ложь).
    static DEPTH: Cell<u8> = const { Cell::new(0) };
}

/// Собрать `@custom-media --имя <запрос>;` верхнего уровня таблицы `css`.
/// Последнее определение имени побеждает (§5.1).
pub(crate) fn scan_custom_media(css: &str) {
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
    CUSTOM.with(|c| *c.borrow_mut() = map);
}

/// Значение `(--имя)`: неизвестное имя — ложь (§5.1 «evaluates to false»).
pub(super) fn eval_custom(media: &Media, name: &str) -> SupTri {
    let Some(query) = CUSTOM.with(|c| c.borrow().get(name).cloned()) else {
        return SupTri::False;
    };
    let v = match query.as_str() {
        "true" => true,
        "false" => false,
        _ if DEPTH.get() < 8 => {
            DEPTH.set(DEPTH.get() + 1);
            let v = media.matches(&query);
            DEPTH.set(DEPTH.get() - 1);
            v
        }
        _ => false,
    };
    if v { SupTri::True } else { SupTri::False }
}

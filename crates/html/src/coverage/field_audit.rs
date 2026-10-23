//! Аудит чтения полей `Computed`: какие поля разрешённого стиля никто не читает.
//!
//! Вход — текст `style/computed/fields.rs` и список исходников-потребителей
//! (`sources::CONSUMERS`); выход — `dead_fields()`, имена полей без единого
//! чтения. Обращение к полю отличается от вызова метода по следующему знаку,
//! а поля, читаемые через методы стиля (`borders()`, `resolve_logical`,
//! `resolve_em` и т. п.), перечислены парой «поле — метод». Отделён от реестра
//! свойств: реестр доказывает разбор, аудит — что разобранное доходит до картинки.

use super::sources::CONSUMERS;

/// Читается ли поле в исходнике.
///
/// Простой поиск подстроки `.имя` обманывался вызовами методов: поле `filter`
/// «читал» любой `.filter(` итератора. Поэтому обращение к полю отличается от
/// вызова метода по следующему символу: у поля дальше не круглая скобка.
pub(super) fn reads_field(source: &str, field: &str) -> bool {
    let needle = format!(".{field}");
    let mut from = 0;
    while let Some(at) = source[from..].find(&needle) {
        let end = from + at + needle.len();
        from = end;
        let next = source[end..].chars().next();
        // Соседняя буква — это другое, более длинное имя.
        if next.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '(') {
            continue;
        }
        return true;
    }
    false
}

/// Поля, которые потребители читают не напрямую, а через метод стиля:
/// `borders()` гасит толщину рамки без рисунка. Поиск по имени поля такой
/// вызов не видит, поэтому пара «поле — метод» перечислена явно.
const ACCESSORS: &[(&str, &str)] = &[
    ("border_width", "borders()"),
    ("border_visible", "borders()"),
    // Логические свойства ложатся на физические поля отдельным проходом
    // сборщика документа (`doc::resolve_logical`).
    ("logical", ".resolve_logical("),
    // `appearance: none` is read through the form-control helper.
    ("appearance", ".appearance_none("),
    // Font-relative исходники разрешаются до создания элементов.
    ("gradient_em", ".resolve_em("),
    ("text_shadow_raw", ".resolve_em("),
    ("transform_raw", ".resolve_em("),
    ("shadow_raw", ".resolve_em("),
    ("transform_origin_raw", ".resolve_em("),
    // Обособление осей читается предикатами: физическая ось зависит ещё и
    // от направления письма.
    ("contain_inline_size", "contains_width()"),
    // `auto` у `max-lines` читается предикатом обрезки по высоте.
    ("clamp_auto", "clamp_by_height()"),
];

/// Поля разрешённого стиля, которые никто не читает.
pub fn dead_fields() -> Vec<String> {
    let source = include_str!("../style/computed/fields.rs");
    let start = match source.find("pub struct Computed {") {
        Some(i) => i,
        None => return vec![],
    };
    let body = &source[start
        ..source[start..]
            .find(
                "
}",
            )
            .map(|e| start + e)
            .unwrap_or(source.len())];
    body.lines()
        .filter_map(|l| l.trim().strip_prefix("pub "))
        .filter_map(|l| l.split(':').next())
        .map(|f| f.trim().to_string())
        .filter(|f| !f.is_empty() && f.chars().all(|c| c.is_ascii_lowercase() || c == '_'))
        .filter(|f| !CONSUMERS.iter().any(|src| reads_field(src, f)))
        .filter(|f| {
            !ACCESSORS
                .iter()
                .any(|(field, call)| field == f && CONSUMERS.iter().any(|src| src.contains(call)))
        })
        .collect()
}

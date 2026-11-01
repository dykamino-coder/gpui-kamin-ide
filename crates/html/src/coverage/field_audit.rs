//! Аудит чтения полей `Computed`: какие поля разрешённого стиля никто не читает.
//!
//! Вход — заголовок и CSS-группы `Computed`, список исходников-потребителей
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

/// Поля `Computed`, видимые до выделения `kamin-style` только внутри крейта
/// (`pub(crate)`): аудит их не проверял и не проверяет. Список только сокращается.
const CRATE_INTERNAL: &[&str] = &[
    "decl_order",
    "side_order",
    "font_weight_step",
    "bidi_inherit",
    "cq_size",
    "quirk_pct_base_hint",
    "stretch_size",
    "will_change_root",
    "backdrop_filter_set",
    "vt_name",
    "color_initial",
    "math_depth",
    "math_font_size",
    "declaration_parse",
    "css_wide_value",
    "mask_border",
    "mask_border_luminance",
    "clamp_continue",
];

/// Листья CSS-групп и факты коробки; ownership handles не являются свойствами.
fn property_fields(source: &str) -> impl Iterator<Item = &str> {
    source.lines().filter_map(|line| {
        let (name, ty) = line.trim().strip_prefix("pub ")?.split_once(':')?;
        if ty.contains("StyleGroup<") || ty.contains("InheritedGroup") || ty.contains("CloneAudit<")
        {
            return None;
        }
        let name = name.trim();
        (!name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
            .then_some(name)
    })
}

/// Поля разрешённого стиля, которые никто не читает.
pub fn dead_fields() -> Vec<String> {
    super::field_sources::SOURCES
        .iter()
        .flat_map(|source| property_fields(source))
        .filter(|f| !CRATE_INTERNAL.contains(f))
        .filter(|f| !CONSUMERS.iter().any(|src| reads_field(src, f)))
        .filter(|f| {
            !ACCESSORS
                .iter()
                .any(|(field, call)| field == f && CONSUMERS.iter().any(|src| src.contains(call)))
        })
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_checks_leaf_properties_instead_of_ownership_handles() {
        let source = "pub struct Style {\n pub reset_border: StyleGroup<Border>,\n pub inh: InheritedGroup,\n pub clone_audit: CloneAudit<false>,\n pub decor_box: u64,\n}\npub struct Border {\n pub border_width: Sides,\n pub radius2: f32,\n}";
        assert_eq!(
            property_fields(source).collect::<Vec<_>>(),
            ["decor_box", "border_width", "radius2"]
        );
        assert!(!reads_field("style.reset_border.border_width", "radius2"));
        assert!(reads_field(
            "style.reset_border.border_width",
            "border_width"
        ));
    }

    #[test]
    fn every_group_contributes_its_css_leaves() {
        let fields: Vec<_> = super::super::field_sources::SOURCES
            .iter()
            .flat_map(|s| property_fields(s))
            .collect();
        assert!(fields.len() >= 528);
        assert!(fields.contains(&"border_width") && fields.contains(&"font_families"));
    }
}

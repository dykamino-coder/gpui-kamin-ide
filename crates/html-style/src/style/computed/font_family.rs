//! Вычисленный стиль: разбор `font-family`. Выбирается первое ДОСТУПНОЕ
//! семейство списка (css-fonts-4 §first-available-font: установленное или
//! подменённое `@font-face`, с U+0020 в unicode-range) — от него считаются метрики; при
//! его отсутствии подставляется родовое или шрифт документа (пустое имя). Весь
//! авторский список сохраняется (`font_families`) для подбора недостающих
//! глифов (CSS Fonts 4 §5.2). Имена проверяются и нормализуются по CSS2 §15.3.

use super::{Computed, family_name_ok, generic_family, is_generic, is_quote};

pub(super) fn apply(style: &mut Computed, v: &str, vcx: crate::values::ValueCx<'_>) {
    if v.eq_ignore_ascii_case("inherit") {
        style.inh.font.font_family = None;
        style.inh.font.font_families = None;
        style.inh.font.monospace = None;
        return;
    }
    if !v.split(',').all(|part| family_name_ok(part.trim())) {
        return;
    }
    style.inh.font.font_families = accept_parsed!(style, Some(families(v, vcx)));
    let registry = vcx.doc.font_snapshot();
    // CSS Fonts 4 sections 2.1 and 5.2: generic and named families occupy
    // the same ordered list. A later installed name cannot outrank a generic.
    for raw in v.split(',').map(str::trim) {
        let lower = raw.to_ascii_lowercase();
        if !raw.starts_with(['\'', '"']) && is_generic(&lower) {
            style.inh.font.monospace = Some(lower == "monospace");
            style.inh.font.font_family = generic_family(&lower).map(str::to_owned);
            return;
        }
        let name = family_name(raw);
        if let Some(found) = registry.first_available_named([name.as_str()]) {
            style.inh.font.font_family = Some(found.to_owned());
            let lower = found.to_ascii_lowercase();
            style.inh.font.monospace = Some(lower.contains("mono") || lower.contains("courier"));
            return;
        }
        // Before platform discovery, preserve the first named request. Once
        // discovery is complete, unavailable names continue to the next entry.
        if !registry.fonts_known() {
            style.inh.font.monospace = Some(lower.contains("mono") || lower.contains("courier"));
            style.inh.font.font_family = Some(name);
            return;
        }
    }
    // No available family: explicitly use the document font, rather than
    // inheriting the parent's family or asking the backend for an absent name.
    style.inh.font.font_family = Some(String::new());
    style.inh.font.monospace = Some(false);
}

fn families(value: &str, vcx: crate::values::ValueCx<'_>) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .map(|name| {
            if !name.starts_with(['\'', '"']) {
                let lower = name.to_ascii_lowercase();
                if lower == "monospace" {
                    return vcx.doc.font_snapshot().mono_family(None).to_string();
                }
                if let Some(family) = generic_family(&lower) {
                    return family.to_string();
                }
            }
            let name = family_name(name);
            vcx.doc
                .font_snapshot()
                .first_available_named([name.as_str()])
                .unwrap_or(&name)
                .to_owned()
        })
        .collect()
}

/// Имя семейства из элемента списка: строка в кавычках — буквально (только
/// экранирование), ряд идентификаторов — через один пробел (CSS2 §15.3).
fn family_name(raw: &str) -> String {
    let un = crate::style::css::unescape(raw.trim_matches(is_quote));
    if raw.starts_with(['\'', '"']) {
        return un;
    }
    un.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn inherit(target: &mut Computed, own: &Computed, parent: &Computed) {
    target.inh.font.font_family = own.inh.font.font_family.clone().or_else(|| {
        own.inh
            .font
            .font_families
            .is_none()
            .then(|| parent.inh.font.font_family.clone())
            .flatten()
    });
    target.inh.font.font_families = own
        .inh
        .font
        .font_families
        .clone()
        .or_else(|| parent.inh.font.font_families.clone());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::css::Media;
    use crate::values::DocumentValues;
    use std::rc::Rc;

    fn document() -> Rc<DocumentValues> {
        let doc = DocumentValues::detached(Media::default());
        doc.set_installed_fonts(Rc::new(
            ["arial", "times new roman", "courier new", "ahem"]
                .map(str::to_owned)
                .into_iter()
                .collect(),
        ));
        doc
    }

    fn family(doc: &DocumentValues, value: &str) -> Computed {
        let mut style = Computed::default();
        apply(&mut style, value, doc.values());
        style
    }

    #[test]
    fn generics_and_named_faces_share_the_authored_order() {
        let doc = document();
        for (value, expected) in [
            ("serif, Arial", Some("Times New Roman")),
            ("Arial, serif", Some("Arial")),
            ("Absent, serif, Arial", Some("Times New Roman")),
            ("'serif', Arial", Some("Arial")),
            ("monospace, Arial", None),
            ("Arial, monospace", Some("Arial")),
            ("ui-rounded, Arial", Some("Arial")),
        ] {
            let style = family(&doc, value);
            assert_eq!(style.inh.font.font_family.as_deref(), expected, "{value}");
            assert_eq!(
                style.inh.font.monospace,
                Some(value.starts_with("monospace")),
                "{value}"
            );
        }
    }

    #[test]
    fn platform_equivalents_precede_later_families_and_reach_glyph_fallback() {
        let doc = document();
        for (value, expected) in [
            ("Helvetica, serif", "Arial"),
            ("'hElVeTiCa', Ahem", "Arial"),
            ("Times, Arial", "Times New Roman"),
            ("Courier, serif", "Courier New"),
        ] {
            let style = family(&doc, value);
            assert_eq!(
                style.inh.font.font_family.as_deref(),
                Some(expected),
                "{value}"
            );
            assert_eq!(
                style.inh.font.font_families.as_ref().unwrap()[0],
                expected,
                "{value}"
            );
        }
    }

    #[test]
    fn document_faces_shadow_platform_equivalents_and_keep_space_coverage() {
        let doc = document();
        doc.font_faces
            .aliases
            .borrow_mut()
            .insert("helvetica".into(), "Ahem".into());
        let style = family(&doc, "Helvetica, serif");
        assert_eq!(style.inh.font.font_family.as_deref(), Some("Helvetica"));
        doc.font_faces
            .has_space
            .borrow_mut()
            .insert("helvetica".into(), false);
        doc.font_registry_changed();
        let style = family(&doc, "Helvetica, serif");
        assert_eq!(
            style.inh.font.font_family.as_deref(),
            Some("Times New Roman")
        );
        let style = family(&doc, "Ahem, Helvetica");
        assert_eq!(style.inh.font.font_family.as_deref(), Some("Ahem"));
        assert_eq!(style.inh.font.font_families.unwrap()[1], "Helvetica");
    }
}

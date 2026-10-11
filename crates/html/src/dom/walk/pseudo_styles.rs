//! Pseudo styles for walk; split out to keep the owning module within 250 lines.

use super::initial_pseudos;
use crate::style::computed::{Computed, Display};
use crate::style::css::{Decls, Rule};
use crate::style::select::matching::matches_ignoring_pseudo;
use crate::style::select::{Ancestor, Sibs};

#[allow(clippy::too_many_arguments)]
#[allow(clippy::needless_borrow)]
pub(super) fn pseudo_styles(
    style: &mut Computed,
    tag: &String,
    rules: &[Rule],
    vars: &Decls,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
) -> (
    Option<Computed>,
    Option<Computed>,
    Option<Computed>,
    Option<Computed>,
) {
    let mut hovered: Vec<&Rule> = rules
        .iter()
        .filter(|r| r.sel.pseudo.as_deref() == Some("hover"))
        .filter(|r| matches_ignoring_pseudo(&r.sel, &me, path, sibs))
        .collect();
    hovered.sort_by_key(|r| (r.sel.specificity(), r.order));
    // Слой наведения собирается ПОВЕРХ базового стиля, а не с нуля:
    // правило `:hover` меняет два-три свойства, а не весь стиль. Со
    // сборкой «с нуля» плавный переход на середине пути показывал
    // голый огрызок — без отступов, размеров и шрифта.
    let hover = (!hovered.is_empty()).then(|| {
        let mut merged = style.clone();
        for rule in hovered.iter() {
            merged.apply_decls_with_vars(&rule.decls, vars);
        }
        merged
    });
    // Псевдоэлементы первой буквы и первой строки — тем же слоем
    // поверх базового стиля: они меняют начертание куска, а не блок.
    let layer = |name, base| initial_pseudos::resolve(name, rules, vars, &me, path, sibs, base);
    let first_letter = layer("first-letter", Some(&style));
    let first_line = layer("first-line", Some(&style));
    let first_line_own = layer("first-line", None).map(Box::new);
    style.first_letter_own = layer("first-letter", None).map(Box::new);
    style.first_line_own = first_line_own;
    // `::marker` — НЕ копией стиля хозяина, как первая буква, а
    // ТОЛЬКО своими объявлениями поверх таблицы агента: копия
    // протащила бы в маркер рамку, поля и размеры самого `<li>`.
    // CSS Lists 3 §3.1.1 gives marker text its own transform default,
    // including when no author ::marker rule matches.
    // Сворачивается ниже, ПОСЛЕ снятия номера пункта:
    // `counter(list-item)` в его `content` обязан видеть своё
    // значение.
    let marker_layer = {
        let mut found: Vec<&Rule> = rules
            .iter()
            .filter(|r| r.sel.pseudo.as_deref() == Some("marker"))
            .filter(|r| matches_ignoring_pseudo(&r.sel, &me, path, sibs))
            .collect();
        found.sort_by_key(|r| (r.sel.specificity(), r.order));
        (!found.is_empty() || tag == "li" || style.display == Some(Display::ListItem)).then(|| {
            let mut m = Computed::default();
            m.text_transform = Some(crate::style::computed::TextTransform::None);
            if !found.is_empty() {
                m.bidi_isolate = Some(true);
            }
            for rule in found.iter() {
                m.apply_decls_with_vars(&rule.decls, vars);
            }
            m
        })
    };

    (hover, first_letter, first_line, marker_layer)
}

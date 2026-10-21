//! Слои стиля элемента при обходе: `:hover`, `::first-letter`, `::first-line`,
//! `::marker`.
//! Слой наведения и слои первой буквы/строки собираются копией базового стиля
//! с наложенными правилами (а не с нуля — иначе переход на середине пути
//! показывал бы голый стиль); «свои» варианты без базы — в
//! `first_letter_own`/`first_line_own`. `::marker` — только собственные
//! объявления поверх умолчаний маркера (css-lists-3 §3.1.1), без копии стиля
//! `<li>`; его `content` сворачивает `walk` уже после снятия номера пункта.

use super::initial_pseudos;
use crate::document::state::ValueCx;
use crate::style::computed::{Computed, Display};
use crate::style::css::{Decls, Rule};
use crate::style::select::matching::matches_ignoring_pseudo;
use crate::style::select::{Ancestor, Sibs};

#[allow(clippy::too_many_arguments)]
#[allow(clippy::needless_borrow)]
pub(super) fn pseudo_styles(
    style: &mut Computed,
    tag: &str,
    rules: &[Rule],
    vars: &Decls,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
    vcx: ValueCx<'_>,
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
            merged.apply_decls_with_vars(&rule.decls, vars, vcx);
        }
        merged
    });
    // Псевдоэлементы первой буквы и первой строки — тем же слоем
    // поверх базового стиля: они меняют начертание куска, а не блок.
    let layer =
        |name, base| initial_pseudos::resolve(name, rules, vars, &me, path, sibs, base, vcx);
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
            // CSS Lists 3 marker UA style preserves spaces and tab characters.
            let mut m = Computed {
                keep_spaces: Some(true),
                preserve_newlines: Some(true),
                nowrap: Some(true),
                text_transform: Some(crate::style::computed::TextTransform::None),
                ..Default::default()
            };
            if !found.is_empty() {
                m.bidi_isolate = Some(true);
            }
            for rule in found.iter() {
                m.apply_decls_with_vars(&rule.decls, vars, vcx);
            }
            m
        })
    };

    (hover, first_letter, first_line, marker_layer)
}

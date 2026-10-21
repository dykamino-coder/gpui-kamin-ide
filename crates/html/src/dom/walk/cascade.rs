//! Каскад одного элемента при обходе: от правил и атрибутов к `Computed`.
//! `cascade_element` отмечает ссылку-маркер группы `scroll-target-group`,
//! отбирает совпавшие правила (документ, умолчания MathML, `::slotted` из
//! `TreeIndex`, `:host` из тени хоста), считает собственные пользовательские
//! свойства поверх родительских, выбирает используемую схему цвета
//! (`color-scheme`; её значение возвращается вызывающему на время поддерева),
//! добавляет представленческие подсказки (HTML и MathML), разрешает стиль, при
//! `appearance: base` докладывает лист форм, затем наследует `content`/счётчики
//! по DOM. Типизированный `attr()` видит атрибуты только этого элемента.

use super::presentational_hints;
use crate::dom::tree_index::TreeIndex;
use crate::dom::*;
use crate::style::cascade::vars::CascadeCx;
use crate::style::computed::Computed;
use crate::style::css::{Decls, Rule, parse_decls};
use crate::style::select::matching::matches;
use crate::style::select::{Ancestor, Sibs, Spot};
use markup5ever_rcdom::Handle;
use std::rc::Rc;

#[allow(clippy::too_many_arguments)]
#[allow(clippy::needless_borrow)]
pub(super) fn cascade_element(
    me: &mut Ancestor,
    attrs: &Vec<(String, String)>,
    tag: &str,
    handle: &Handle,
    tree: &TreeIndex,
    rules: &[Rule],
    inherited_vars: &Decls,
    path: &[Ancestor],
    spot: Spot,
    sibs: Sibs,
    vcx: crate::document::state::ValueCx<'_>,
) -> (
    Computed,
    Decls,
    crate::style::values::value::Scheme,
    Option<Rc<crate::dom::shadow::Shadow>>,
) {
    // css-overflow-5 §scroll-target-group: links with a fragment URL inside a
    // `scroll-target-group: auto` element are its scroll markers. At the
    // initial scroll position the first target in tree order is current
    // (Blink `ScrollMarkerGroupData` picks the target nearest the start).
    if me.tag == "a"
        && me
            .href
            .as_deref()
            .is_some_and(|h| h.len() > 1 && h.starts_with('#'))
        && let Some(group) = path.iter().rev().find_map(|a| a.target_group.as_ref())
        && !group.replace(true)
    {
        me.target_current = true;
    }
    let vars = inherited_vars;
    let inline_decls: Decls = attrs
        .iter()
        .find(|(k, _)| k == "style")
        .map(|(_, v)| parse_decls(v))
        .unwrap_or_default();
    let mut matched: Vec<&Rule> = rules
        .iter()
        .filter(|r| matches(&r.sel, &me, path, sibs))
        .collect();
    let math_defaults = crate::dom::mathml::defaults(handle, spot.index);
    matched.extend(math_defaults.iter());
    let slotted = tree.slotted_rules_of(handle);
    matched.extend(slotted.iter());
    // Правила `:host`/`:host(S)` из ТЕНИ хоста ложатся на сам хост
    // (css-shadow-1 §3.1; Blink `MatchHostRules`). Хост для них
    // безлик: совпадает только `:host`-компаунд без предков и братьев.
    let shadow = tree.shadow_of(handle);
    if let Some(shadow) = &shadow {
        matched.extend(
            shadow
                .scope
                .rules
                .iter()
                .filter(|r| matches(&r.sel, &shadow.marker, &[], Sibs::EMPTY)),
        );
    }
    // Свои переменные: родительские, поверх них объявления
    // совпавших правил в порядке каскада, поверх — свои же в
    // атрибуте. Пока словарь был один на документ, `:root{--c:red}`
    // и `.dark{--c:blue}` складывались в него подряд, и последнее
    // объявление красило ВЕСЬ документ — переключение темы классом
    // не работало в принципе.
    let registered = tree.doc.property_rules();
    let cascaded = crate::style::css::custom_properties::cascade(
        &matched,
        &inline_decls,
        vars,
        &registered,
        |s, v| syntax_accepts(s, v, vcx),
    );
    let own_vars =
        crate::style::css::variable_values::compute(&cascaded, vars, &registered, |s, v| {
            syntax_accepts(s, v, vcx)
        });
    let vars = &own_vars;
    // Используемая схема цвета (css-color-adjust-1 §color-scheme-prop):
    // своё `color-scheme` — последнее по каскаду, иначе родительская
    // (свойство наследуемое). При `light dark` берётся предпочтение
    // текущего документа, включая схему embedding element. Держится на время узла и его потомков.
    let scheme = {
        let mut by_cascade: Vec<&&Rule> = matched.iter().collect();
        by_cascade.sort_by(|a, b| {
            (a.origin, &a.layer, a.sel.specificity(), a.order).cmp(&(
                b.origin,
                &b.layer,
                b.sel.specificity(),
                b.order,
            ))
        });
        let mut last: Option<String> = None;
        for rule in by_cascade {
            if let Some(v) = rule.decls.get("color-scheme") {
                last = Some(v.clone());
            }
        }
        if let Some(v) = inline_decls.get("color-scheme") {
            last = Some(v.clone());
        }
        last
    };
    let vcx = scheme
        .as_deref()
        .map_or(vcx, |v| crate::style::values::value::resolve_scheme(v, vcx));
    // Типизированный `attr()` читает атрибуты ЭТОГО элемента
    // (css-values-5 §7.7), `sibling-index()` — его место среди братьев.
    let element = CascadeCx {
        attrs,
        sibling: (spot.index > 0).then_some((spot.index, spot.total)),
    };
    let hints = presentational_hints::rules(&tag, &attrs, path, vcx);
    matched.extend(hints.iter());
    let math_hints = crate::dom::mathml::hints(handle, attrs, vcx);
    matched.extend(math_hints.iter());
    let mut style = Computed::resolve_in(&mut matched, &inline_decls, vars, element, vcx);
    if let Some(basic) = super::form_appearance::rule(&tag, &attrs, &style) {
        matched.push(&basic);
        style = Computed::resolve_in(&mut matched, &inline_decls, vars, element, vcx);
    }
    style.mathml = crate::dom::mathml::is_mathml(handle);
    if crate::dom::mathml_routing::enabled(handle) {
        if let Some(display) = style.math_layout {
            style.display = Some(display);
        }
    } else {
        style.math_layout = None;
    }
    style.dark_color_scheme = crate::style::values::value::dark_scheme(vcx);
    style.embedding_dark_scheme = crate::style::values::value::embedding_dark_scheme(vcx);
    style.custom_scrollbar = rules.iter().any(|rule| {
        if rule.sel.pseudo.as_deref() != Some("-webkit-scrollbar") {
            return false;
        }
        let mut owner = rule.sel.clone();
        owner.pseudo = None;
        matches(&owner, me, path, sibs)
    });
    content::inherit_generated(&mut style, path.last());
    apply_value_hint(&mut style, &me);
    (style, own_vars, vcx.scheme, shadow)
}

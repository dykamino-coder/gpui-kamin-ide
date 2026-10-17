//! Cascade for walk; split out to keep the owning module within 250 lines.

use super::presentational_hints;
use crate::dom::*;
use crate::style::computed::Computed;
use crate::style::css::{Decls, Rule, parse_decls};
use crate::style::select::matching::matches;
use crate::style::select::{Ancestor, Sibs, Spot};
use markup5ever_rcdom::Handle;
use std::rc::Rc;

pub(crate) struct SchemeGuard((bool, bool));

impl Drop for SchemeGuard {
    fn drop(&mut self) {
        crate::style::values::value::restore_scheme(self.0);
    }
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::needless_borrow)]
pub(super) fn cascade_element(
    me: &mut Ancestor,
    attrs: &Vec<(String, String)>,
    tag: &String,
    handle: &Handle,
    rules: &[Rule],
    inherited_vars: &Decls,
    path: &[Ancestor],
    spot: Spot,
    sibs: Sibs,
) -> (
    Computed,
    Decls,
    SchemeGuard,
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
    let slotted = crate::dom::shadow::slotted::rules_of(handle);
    matched.extend(slotted.iter());
    // Правила `:host`/`:host(S)` из ТЕНИ хоста ложатся на сам хост
    // (css-shadow-1 §3.1; Blink `MatchHostRules`). Хост для них
    // безлик: совпадает только `:host`-компаунд без предков и братьев.
    let shadow = shadow_of(handle);
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
    let registered = crate::style::css::property_rules();
    let cascaded = crate::style::css::custom_properties::cascade(
        &matched,
        &inline_decls,
        vars,
        &registered,
        syntax_accepts,
    );
    let own_vars =
        crate::style::css::variable_values::compute(&cascaded, vars, &registered, syntax_accepts);
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
    let _scheme = SchemeGuard(crate::style::values::value::scheme_state());
    if let Some(v) = scheme {
        crate::style::values::value::resolve_scheme(&v);
    }
    // Типизированный `attr()` читает атрибуты ЭТОГО элемента
    // (css-values-5 §7.7): слот ставится только на время его каскада.
    crate::style::cascade::vars::set_current_attrs(&attrs);
    crate::style::cascade::vars::set_current_sibling(
        (spot.index > 0).then_some((spot.index, spot.total)),
    );
    let hints = presentational_hints::rules(&tag, &attrs, path);
    matched.extend(hints.iter());
    let math_hints = crate::dom::mathml::hints(handle, attrs);
    matched.extend(math_hints.iter());
    let mut style = Computed::resolve_with_vars(&mut matched, &inline_decls, vars);
    if let Some(basic) = super::form_appearance::rule(&tag, &attrs, &style) {
        matched.push(&basic);
        style = Computed::resolve_with_vars(&mut matched, &inline_decls, vars);
    }
    style.mathml = crate::dom::mathml::is_mathml(handle);
    if crate::dom::mathml_routing::enabled(handle) {
        if let Some(display) = style.math_layout {
            style.display = Some(display);
        }
    } else {
        style.math_layout = None;
    }
    style.dark_color_scheme = crate::style::values::value::dark_scheme();
    style.embedding_dark_scheme = crate::style::values::value::embedding_dark_scheme();
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
    crate::style::cascade::vars::clear_current_attrs();
    crate::style::cascade::vars::set_current_sibling(None);
    (style, own_vars, _scheme, shadow)
}

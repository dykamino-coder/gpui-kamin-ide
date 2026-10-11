//! Cascade for walk; split out to keep the owning module within 250 lines.

use super::presentational_hints;
use crate::dom::*;
use crate::style::computed::Computed;
use crate::style::css::{Decls, Rule, parse_decls};
use crate::style::select::matching::matches;
use crate::style::select::{Ancestor, Sibs, Spot};
use markup5ever_rcdom::Handle;
use std::rc::Rc;

pub(crate) struct SchemeGuard(bool);

impl Drop for SchemeGuard {
    fn drop(&mut self) {
        crate::style::values::value::set_dark_scheme(self.0);
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
    // (свойство наследуемое). Тёмная — когда названа только `dark`:
    // при `light dark` берётся предпочтение пользователя, у стенда
    // светлое. Держится на время узла и его потомков.
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
    let parent_dark = crate::style::values::value::dark_scheme();
    let _scheme = SchemeGuard(parent_dark);
    if let Some(v) = scheme {
        let low = v.to_ascii_lowercase();
        let words: Vec<&str> = low.split_whitespace().collect();
        let dark = words.contains(&"dark") && !words.contains(&"light");
        if !low.contains("inherit") {
            crate::style::values::value::set_dark_scheme(dark);
        }
    }
    // Типизированный `attr()` читает атрибуты ЭТОГО элемента
    // (css-values-5 §7.7): слот ставится только на время его каскада.
    crate::style::cascade::vars::set_current_attrs(&attrs);
    crate::style::cascade::vars::set_current_sibling(
        (spot.index > 0).then_some((spot.index, spot.total)),
    );
    let hints = presentational_hints::rules(&tag, &attrs);
    matched.extend(hints.iter());
    let mut style = Computed::resolve_with_vars(&mut matched, &inline_decls, vars);
    inherit_counter_decls(&mut style, path.last().map(|p| &p.counter_style));
    apply_value_hint(&mut style, &me);
    me.counter_style = counter_snapshot(&style);
    crate::style::cascade::vars::clear_current_attrs();
    crate::style::cascade::vars::set_current_sibling(None);
    (style, own_vars, _scheme, shadow)
}

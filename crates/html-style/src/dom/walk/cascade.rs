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
use crate::style::cascade::map::CascadeMap;
use crate::style::cascade::vars::CascadeCx;
use crate::style::computed::Computed;
use crate::style::css::{DeclBlock, LinkMatch, Rule, VarMap, parse_decls_in};
use crate::style::select::matching::matches;
use crate::style::select::{Ancestor, Sibs, Spot};
use markup5ever_rcdom::Handle;
use std::rc::Rc;

#[allow(clippy::too_many_arguments)]
pub(super) fn cascade_element(
    me: &mut Ancestor,
    attrs: &Vec<(String, String)>,
    tag: &str,
    handle: &Handle,
    tree: &TreeIndex,
    scope: &crate::dom::shadow::Scope,
    candidates: &[usize],
    inherited_vars: &VarMap,
    path: &[Ancestor],
    spot: Spot,
    sibs: Sibs,
    vcx: crate::values::ValueCx<'_>,
) -> (
    Box<Computed>,
    VarMap,
    crate::style::values::value::Scheme,
    Option<Rc<crate::dom::shadow::Shadow>>,
    Option<DeclBlock>,
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
    let rules = &scope.rules;
    let vars = inherited_vars;
    let inline_decls: DeclBlock = attrs
        .iter()
        .find(|(k, _)| k == "style")
        .map(|(_, v)| parse_decls_in(v, vcx))
        .unwrap_or_default();
    // Ограничение `:visited`: правило, совпавшее лишь при одном состоянии
    // своей ссылки, входит в каскад копией с этим признаком.
    let link = super::visited_match::link_matches(rules, &scope.index, candidates, me, path, sibs);
    let restricted: Vec<Rule> = link
        .iter()
        .filter(|(_, l)| *l != LinkMatch::Any)
        .map(|&(i, l)| crate::style::css::visited::restrict(&rules[i], l))
        .collect();
    let mut matched: Vec<&Rule> = link
        .iter()
        .filter(|(_, l)| *l == LinkMatch::Any)
        .map(|&(i, _)| &rules[i])
        .chain(restricted.iter())
        .collect();
    let math_defaults = crate::dom::mathml::defaults(handle, spot.index, vcx);
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
                .index
                .candidates(&shadow.marker, &[])
                .iter()
                .map(|&i| &shadow.scope.rules[i])
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
    // Одна карта каскада на пользовательские свойства и `color-scheme`:
    // подсказки и лист форм их не задают.
    let hints = presentational_hints::rules(&tag, &attrs, path, vcx);
    matched.extend(hints.iter());
    let math_hints = crate::dom::mathml::hints(handle, attrs, vcx);
    matched.extend(math_hints.iter());
    let pre = CascadeMap::collect(&matched, &inline_decls);
    let cascaded =
        crate::style::css::custom_properties::cascade_map(&pre, vars, &registered.rules, |s, v| {
            syntax_accepts(s, v, vcx)
        });
    let revision = tree.doc.revision_budget();
    let work = crate::style::css::variable_values::Work::new(&revision);
    let own_vars = crate::style::css::variable_values::compute(
        &cascaded,
        vars,
        &registered.rules,
        |s, v| syntax_accepts(s, v, vcx),
        &work,
    );
    work.finish(&tree.doc);
    let vars = &own_vars;
    // Типизированный `attr()` читает атрибуты ЭТОГО элемента
    // (css-values-5 §7.7), `sibling-index()` — его место среди братьев.
    let element = CascadeCx {
        attrs,
        html_attrs: me.html_attrs,
        sibling: (spot.index > 0).then_some((spot.index, spot.total)),
        root: path.is_empty(),
    };
    let appearance = matches!(tag, "input" | "textarea" | "button")
        .then(|| Computed::appearance_from_map(&pre, vars, element, vcx));
    let basic = appearance
        .as_ref()
        .and_then(|style| super::form_appearance::rule(tag, attrs, style));
    let map = if let Some(basic) = &basic {
        matched.push(basic);
        CascadeMap::collect(&matched, &inline_decls)
    } else {
        pre
    };
    let mut style = tree
        .style_sharing
        .resolve(&matched, &inline_decls, &map, vars, element, vcx);
    // Важные объявления сильнее анимаций (css-cascade-5 §6.2): кадры
    // `@keyframes` кладутся под них. Лист форм
    // (`appearance: base`) важных объявлений не несёт.
    let important = style
        .reset_context
        .animation
        .is_some()
        .then(|| map.important_block());
    // Используемая схема цвета (css-color-adjust-1 §color-scheme-prop):
    // вычисленное `color-scheme` элемента (каскад уже решил им свои цвета,
    // `ComputeCx`), иначе родительская — свойство наследуемое. При
    // `light dark` берётся предпочтение текущего документа, включая схему
    // embedding element. Держится на время узла и его потомков.
    let vcx = style
        .inh
        .ui
        .color_scheme
        .as_deref()
        .map_or(vcx, |v| crate::style::values::value::resolve_scheme(v, vcx));
    let value = crate::dom::mathml::is_mathml(handle);
    if style.reset_svg_math.mathml != value {
        style.reset_svg_math.mathml = value;
    }
    let value = super::ancestor::svg_no_css_box(handle);
    if style.reset_svg_math.svg_no_css_box != value {
        style.reset_svg_math.svg_no_css_box = value;
    }
    if crate::dom::mathml_routing::enabled(handle, vcx) {
        if let Some(display) = style.reset_svg_math.math_layout {
            style.reset_box_layout.display = Some(display);
        }
    } else {
        if style.reset_svg_math.math_layout.is_some() {
            style.reset_svg_math.math_layout = None;
        }
    }
    let value = crate::style::values::value::dark_scheme(vcx);
    if style.reset_cascade.dark_color_scheme != value {
        style.reset_cascade.dark_color_scheme = value;
    }
    let value = crate::style::values::value::embedding_dark_scheme(vcx);
    if style.reset_cascade.embedding_dark_scheme != value {
        style.reset_cascade.embedding_dark_scheme = value;
    }
    // css-color-4 §4.1: the initial `color` is `CanvasText` of the used color
    // scheme. Below the root an empty slot inherits; at the root it is the
    // initial value, resolved against the root's own scheme.
    if element.root && style.inh.ui.color.is_none() && style.reset_cascade.color_function.is_none()
    {
        style.apply_one("color", "canvastext", vcx);
    }
    let custom_scrollbar = candidates.iter().any(|&i| {
        let rule = &rules[i];
        if rule.sel.pseudo.as_deref() != Some("-webkit-scrollbar") {
            return false;
        }
        let mut owner = rule.sel.clone();
        owner.pseudo = None;
        matches(&owner, me, path, sibs)
    });
    if style.reset_box_layout.custom_scrollbar != custom_scrollbar {
        style.reset_box_layout.custom_scrollbar = custom_scrollbar;
    }
    content::inherit_generated(&mut style, path.last());
    apply_value_hint(&mut style, &me);
    (Box::new(style), own_vars, vcx.scheme, shadow, important)
}

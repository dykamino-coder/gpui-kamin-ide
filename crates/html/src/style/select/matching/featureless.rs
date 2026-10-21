//! Сопоставление с безликим хостом (изнутри его тени, Selectors §3.2.1): с ним
//! совпадает только компаунд из `:host`/`:host()`, `:is()`/`:not()` вокруг них
//! и `:has()` по теневому дереву. `None` означает, что компаунд к безликому
//! хосту неприменим, — это не то же, что несовпадение: отрицание не должно
//! превращать запрещённый селектор в совпадение.

use super::{Ancestor, Selector, has_id, host_holds};
use crate::style::css::{TypeNamespace, default_ns, split_selector_list};

/// None means the subject compound is forbidden on a featureless host. In
/// particular, negation must not turn a forbidden selector into a match.
pub(super) fn matches_featureless(sel: &Selector, node: &Ancestor) -> Option<bool> {
    if sel.tag.is_some()
        || sel.universal
        || sel.id.is_some()
        || !sel.classes.is_empty()
        || !sel.attrs.is_empty()
    {
        return None;
    }
    let mut eligible = false;
    let mut hit = true;
    for p in sel.pseudo.iter().chain(&sel.also) {
        if let Some(arg) = p.strip_prefix("has(").and_then(|a| a.strip_suffix(')')) {
            hit &= node.shadow_has_marks.contains(&has_id(arg));
        } else {
            hit &= pseudo(p, sel.argument_namespace, node)?;
            eligible = true;
        }
    }
    eligible.then_some(hit && sel.ancestor.is_none() && sel.prev.is_none())
}

fn pseudo(p: &str, namespace: TypeNamespace, node: &Ancestor) -> Option<bool> {
    if super::is_host_pseudo(p) {
        return Some(host_holds(p, node));
    }
    let (name, arg) = p.split_once('(')?;
    let arg = arg.strip_suffix(')')?;
    let list = match name {
        "is" | "where" | "matches" | "any" => default_ns::is_args(arg, namespace),
        "not" => split_selector_list(arg)
            .into_iter()
            .filter_map(Selector::parse)
            .collect(),
        _ => return None,
    };
    let mut allowed = false;
    let mut hit = false;
    for s in list {
        if let Some(result) = matches_featureless(&s, node) {
            allowed = true;
            hit |= result;
        }
    }
    allowed.then_some(if name == "not" { !hit } else { hit })
}

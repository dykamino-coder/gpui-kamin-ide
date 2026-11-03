//! Сопоставление с безликим хостом (изнутри его тени, Selectors §3.2.1): с ним
//! совпадает только компаунд из `:host`/`:host()`, `:is()`/`:not()` вокруг них
//! и `:has()` по теневому дереву. `None` означает, что компаунд к безликому
//! хосту неприменим, — это не то же, что несовпадение: отрицание не должно
//! превращать запрещённый селектор в совпадение.

use super::{Ancestor, PseudoKind, Selector, host_holds};

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
        if let (_, PseudoKind::Has(id)) = sel.pseudo_of(p) {
            hit &= node.shadow_has_marks.contains(&id);
        } else {
            hit &= pseudo(sel, p, node)?;
            eligible = true;
        }
    }
    eligible.then_some(hit && sel.ancestor.is_none() && sel.prev.is_none())
}

fn pseudo(sel: &Selector, p: &str, node: &Ancestor) -> Option<bool> {
    let (prepared, kind) = sel.pseudo_of(p);
    let negate = match kind {
        PseudoKind::Host => return Some(host_holds(sel, p, node)),
        PseudoKind::Is => false,
        PseudoKind::Not => true,
        _ => return None,
    };
    let list = &prepared?.list;
    let mut allowed = false;
    let mut hit = false;
    for s in list {
        if let Some(result) = matches_featureless(s, node) {
            allowed = true;
            hit |= result;
        }
    }
    allowed.then_some(if negate { !hit } else { hit })
}

//! Корни продолжений legacy-копий и семьи, которые источник не может разместить сам.

use super::*;

/// Outermost absolute/fixed roots of `kids` continued by their native source;
/// boxes inside a nested multicol belong to that container's own source.
pub(crate) fn continuation_roots<'a>(kids: impl Iterator<Item = &'a Element>) -> Rc<[u64]> {
    let mut roots = Vec::new();
    let mut pending: Vec<&Element> = kids.collect();
    while let Some(e) = pending.pop() {
        if positioned(e.style.used().position) {
            // A held box keeps its family in the copies, as its source
            // snapshot does (fragmentation::snapshot).
            if !holds_held(e) {
                roots.push(e.node_id);
            }
            continue;
        }
        if crate::layout::multicol::spanner::multicol_container(&e.style.used()) {
            continue;
        }
        pending.extend(e.children.iter().filter_map(|n| match n {
            Node::Element(c) => Some(c),
            _ => None,
        }));
    }
    Rc::from(roots)
}

/// The source cannot place `e` itself (see `Flow::held`). A box transformed
/// beyond a translation resolves its transform origin per fragment, after its
/// fragment is cut (css-break-3 §2, css-transforms-1 §transform-rendering);
/// one capture bakes a single origin and its admission cuts after the matrix.
pub(super) fn held(e: &Element) -> bool {
    let used = e.style.used();
    let linear = used.transform.as_ref().is_some_and(|t| {
        let [[a, b], [c, d]] = t.lin;
        let eps = 1e-5;
        (a - 1.).abs() > eps
            || b.abs() > eps
            || c.abs() > eps
            || (d - 1.).abs() > eps
            || (0..4).any(|i| {
                (0..4).any(|j| {
                    let identity = if i == j { 1. } else { 0. };
                    j != 3 && (t.m4[i][j] - identity).abs() > eps
                })
            })
    });
    linear
        || crate::layout::positioned::anchor::resolve::needs_deferred(&used)
        || crate::layout::positioned::static_position::static_place_in_flow_only(&used)
}

/// `e` or a positioned box in its family is held (see `Flow::held`).
fn holds_held(e: &Element) -> bool {
    let mut pending = vec![e];
    while let Some(e) = pending.pop() {
        let used = e.style.used();
        if positioned(used.position) && held(e) {
            return true;
        }
        if crate::layout::multicol::spanner::multicol_container(&used) {
            continue;
        }
        pending.extend(e.children.iter().filter_map(|n| match n {
            Node::Element(c) => Some(c),
            _ => None,
        }));
    }
    false
}

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

/// The source cannot place `e` itself (`Flow::held`): an anchor-positioned
/// box takes its place from anchors in the normal flow, which the hybrid lays
/// out in the legacy copies (P21 moves normal flow to the native source).
pub(super) fn held(e: &Element) -> bool {
    crate::layout::positioned::anchor::resolve::needs_deferred(&e.style.used())
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

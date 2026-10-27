//! Корни продолжений legacy-копий и семьи, которые источник не может разместить сам.

use super::*;

/// Outermost absolute/fixed roots of `kids` continued by their native source;
/// boxes inside a nested multicol belong to that container's own source.
pub(crate) fn continuation_roots<'a>(kids: impl Iterator<Item = &'a Element>) -> Rc<[u64]> {
    let mut roots = Vec::new();
    let mut pending: Vec<&Element> = kids.collect();
    while let Some(e) = pending.pop() {
        if positioned(e.style.used().position) {
            roots.push(e.node_id);
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

/// The source cannot resolve `e`'s place itself (`Flow::follows`): an anchor
/// laid out elsewhere places it. Its record follows the copy that resolved it.
pub(super) fn follows(e: &Element) -> bool {
    crate::layout::positioned::anchor::resolve::needs_deferred(&e.style.used())
}

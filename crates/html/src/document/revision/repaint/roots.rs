//! The roots of the dirty subtrees of a batch (`LayoutRoot`) and the one
//! fallback of the style level (`fall_back`).
//!
//! Blink marks the layout object of each element whose `StyleDifference`
//! needs layout and walks the marks up to the nearest relayout boundary
//! (`LayoutObject::SetNeedsLayout`, `MarkContainerChainForLayout`); a box
//! tree change reattaches the element's subtree (`Element::RebuildLayoutTree`).
//! Here the damaged elements of a batch reduce to the roots of their
//! subtrees: an element under a damaged ancestor is part of that ancestor's
//! subtree. Layout of the roots over the retained box tree and the rebuild of
//! a root's boxes are the executors of these levels; until they exist, a
//! batch with such roots is rebuilt here, at this one point.

use super::levels::{Deeper, Levels};
use crate::document::revision::boxes::Boxes;
use crate::document::revision::patch::Path;
use crate::document::revision::source_index::SourceIndex;
use crate::paint_path as why;
use crate::style::computed::Damage;
use markup5ever_rcdom::Handle;
use std::collections::HashSet;
use std::rc::Rc;

/// The root of a dirty subtree: the boxes of one element whose damage
/// reaches layout, with every damaged element under it.
pub(in crate::document) struct LayoutRoot {
    /// The node id of the element's boxes; `None` for the document.
    pub(in crate::document) node: Option<u64>,
    /// The paths of the element's boxes in the current tree, in tree order.
    pub(in crate::document) paths: Vec<Path>,
    /// Some element of the subtree changes its boxes (`Damage::Rebox`).
    pub(in crate::document) rebox: bool,
}

/// The roots of the subtrees of `deeper`, in tree order of their first box.
pub(super) fn roots(deeper: &[Deeper], index: &SourceIndex, boxes: &Boxes) -> Vec<LayoutRoot> {
    let marked: HashSet<usize> = deeper.iter().map(|d| address(&d.handle)).collect();
    let mut tops: Vec<(Handle, bool)> = Vec::new();
    for d in deeper {
        let top = top_marked(&d.handle, &marked);
        let rebox = d.damage == Damage::Rebox;
        match tops.iter_mut().find(|(t, _)| Rc::ptr_eq(t, &top)) {
            Some((_, r)) => *r |= rebox,
            None => tops.push((top, rebox)),
        }
    }
    let mut roots: Vec<LayoutRoot> = tops
        .into_iter()
        .map(|(top, rebox)| {
            let node = index.id(&top);
            let paths = node.map(|id| boxes.paths(id).collect()).unwrap_or_default();
            LayoutRoot { node, paths, rebox }
        })
        .collect();
    roots.sort_by(|a, b| a.paths.first().cmp(&b.paths.first()));
    roots
}

fn address(handle: &Handle) -> usize {
    Rc::as_ptr(handle) as usize
}

/// The topmost marked ancestor-or-self of the marked `handle`.
fn top_marked(handle: &Handle, marked: &HashSet<usize>) -> Handle {
    let mut top = handle.clone();
    let mut at = parent(handle);
    while let Some(node) = at {
        if marked.contains(&address(&node)) {
            top = node.clone();
        }
        at = parent(&node);
    }
    top
}
fn parent(node: &Handle) -> Option<Handle> {
    let weak = node.parent.take();
    let parent = weak
        .as_ref()
        .and_then(markup5ever_rcdom::WeakHandle::upgrade);
    node.parent.set(weak);
    parent
}

/// The one fallback of the style level: a batch whose damage reaches layout
/// or the box tree is rebuilt, with its roots counted and journaled.
pub(super) fn fall_back<T>(levels: &Levels, index: &SourceIndex, boxes: &Boxes) -> Option<T> {
    let layout = levels
        .restyles
        .values()
        .filter(|r| r.damage.0 == Damage::Layout);
    let layout = layout.map(|r| Deeper {
        handle: r.handle.clone(),
        damage: Damage::Layout,
        why: r.damage.1,
    });
    let deeper: Vec<Deeper> = layout.chain(levels.deeper.iter().cloned()).collect();
    let roots = roots(&deeper, index, boxes);
    let rebox = roots.iter().filter(|r| r.rebox).count();
    let layout = roots.len() - rebox;
    let paint = levels
        .restyles
        .values()
        .filter(|r| r.damage.0 == Damage::Paint);
    why::add(&why::DAMAGE_PAINT, paint.count() as u64);
    rebuild(layout, rebox, || {
        let mut causes: Vec<&str> = deeper.iter().map(|d| d.why).collect();
        causes.sort_unstable();
        causes.dedup();
        let boxes: usize = roots.iter().map(|r| r.paths.len()).sum();
        let document = roots.iter().any(|r| r.node.is_none());
        let document = if document { ", the document" } else { "" };
        format!("({boxes} boxes{document}): {}", causes.join(", "))
    })
}

/// The fallback for a batch one of whose boxes cannot take its change in
/// place (or, before any element is classified, the whole document): it is
/// a root whose boxes are rebuilt (`Damage::Rebox`).
pub(in crate::document::revision) fn fall_back_rebox<T>(cause: &'static str) -> Option<T> {
    rebuild(0, 1, || format!(": {cause}"))
}

/// The rebuild itself: one journal point for every batch the style level
/// hands to a full build.
fn rebuild<T>(layout: usize, rebox: usize, detail: impl FnOnce() -> String) -> Option<T> {
    why::add(&why::DAMAGE_LAYOUT, layout as u64);
    why::add(&why::DAMAGE_REBOX, rebox as u64);
    why::refuse_with(&why::REFUSED_DAMAGE, || {
        format!("{layout} layout roots, {rebox} rebox roots {}", detail())
    });
    None
}

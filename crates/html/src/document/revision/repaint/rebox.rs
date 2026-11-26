//! The executor of the damage that reaches the box tree or layout: the boxes
//! of each root (`roots`) are built again by the walk of a full build under
//! the root's kept context (`walk::subtree`) and the passes of a full build
//! (`spine`), and take the place of the retained boxes in a copy of the
//! tree's paths (`patch::apply`).
//!
//! One rule decides how far a root reaches, with no list of cases: a root
//! is its own unit when
//! 1. its subtree built before the batch is the retained subtree, value for
//!    value (`same_cascaded_tree`): no document pass and no context outside
//!    it (anonymous wrappers, lifted grid boxes, run-ins, the removed
//!    `html`/`body`) made its boxes other than its own walk does;
//! 2. it adds one box of its own to its parent before and after the batch,
//!    from which the parent's walk reads the same values (`outside`), and
//!    the passes of the build give its ancestors and their other children
//!    the same values, neither moving nor wrapping it (`spine`);
//! 3. it reads no counter or quote from the walk before it, and hands the
//!    same updates to the walk after it, or counters whose scope a following
//!    sibling ends (`Counters::isolated`, `outside`).
//!
//! Otherwise its parent element is the root, and the rule applies to it;
//! the document is the last root (`roots::fall_back_document`). Blink
//! rebuilds the layout tree from the nearest ancestor with a layout object
//! and lets the parent recreate its anonymous wrappers
//! (`Element::RebuildLayoutTree`, `LayoutObject::AddChild`); Gecko walks the
//! reconstruction up to a container that holds what the frames reach
//! (`nsCSSFrameConstructor::MaybeRecreateContainerForFrameRemoval`,
//! `WipeContainingBlock`).

mod outside;
mod spine;
mod unit;
mod wrappers;
pub(super) use unit::same_generated;

use crate::document::Document;
use crate::document::revision::boxes::Boxes;
use crate::document::revision::mutation::Batch;
use crate::document::revision::patch::Path;
use crate::document::revision::source_index::SourceIndex;
use crate::dom::Element;
use crate::dom::restyle_probe::{RestyleCache, RestyleProbe};
use markup5ever_rcdom::{Handle, NodeData};
use std::collections::HashSet;
use std::rc::Rc;

/// The rebuilt boxes of one root.
pub(super) struct Unit {
    pub(super) handle: Handle,
    /// The path of the root's box in the retained tree.
    pub(super) path: Path,
    pub(super) node: Element,
    /// The node ids of the elements of the old and the new subtree.
    pub(super) ids: Vec<u64>,
    /// The source elements the walks of the old and the new subtree built.
    dropped: Vec<usize>,
    built: Vec<usize>,
    /// The old or the new subtree holds SVG masks or clip paths, which the
    /// document indexes by id over the whole tree (`masks`).
    pub(super) masks: bool,
}

/// The units of one batch.
pub(super) struct Reboxed(pub(super) Vec<Unit>);

impl Reboxed {
    /// `handle` lies in the subtree of a unit.
    pub(super) fn covers(&self, handle: &Handle) -> bool {
        self.0.iter().any(|u| within(handle, &u.handle))
    }

    /// The batch commits its rebuilt boxes: the build's list of elements
    /// takes those of the new subtrees.
    pub(super) fn commit(&self, values: &crate::values::DocumentValues) {
        let dropped: Vec<usize> = self
            .0
            .iter()
            .flat_map(|u| u.dropped.iter().copied())
            .collect();
        let built: Vec<usize> = self
            .0
            .iter()
            .flat_map(|u| u.built.iter().copied())
            .collect();
        values.traversal.end_rebuild(&dropped, &built);
    }
}

/// What a walk of the roots reads.
pub(super) struct Walks<'a> {
    pub(super) probe: &'a RestyleProbe,
    pub(super) input: &'a crate::dom::input::Admitted,
    pub(super) index: &'a SourceIndex,
    pub(super) boxes: &'a Boxes,
    /// Elements whose style the batch changes: a root under one of them
    /// takes it as its root (its values reach the subtree).
    pub(super) restyled: &'a HashSet<usize>,
    /// The elements of the batch.
    pub(super) mutated: &'a [Handle],
}

impl Document {
    /// The units of the box executor for `roots` (`rebox`); `Err` names
    /// why the document is the root.
    pub(super) fn reboxed(
        &self,
        batch: &mut Batch,
        roots: Vec<markup5ever_rcdom::Handle>,
        levels: &super::levels::Levels,
        (input, index, boxes): (&crate::dom::input::Admitted, &SourceIndex, &Boxes),
        theme_css: &str,
        kept: &mut super::Kept,
    ) -> Result<Reboxed, &'static str> {
        if let Some(d) = levels
            .deeper
            .iter()
            .find(|d| !matches!(d.handle.data, markup5ever_rcdom::NodeData::Element { .. }))
        {
            return Err(d.why);
        }
        let media = self.state.media();
        let values = &self.state.values;
        let Ok(probe) = kept.cache.probe(input, theme_css, media, values) else {
            return Err("probe");
        };
        let address = |h: &markup5ever_rcdom::Handle| std::rc::Rc::as_ptr(h) as usize;
        let restyled = levels.restyles.values().map(|r| address(&r.handle));
        let restyled = restyled.chain(levels.deeper.iter().map(|d| address(&d.handle)));
        let mutated: Vec<Handle> = batch.elements.iter().map(|c| c.element.clone()).collect();
        let walks = Walks {
            probe: &probe,
            input,
            index,
            boxes,
            restyled: &restyled.collect(),
            mutated: &mutated,
        };
        self.rebox(batch, roots, &walks, &mut kept.cache)
    }

    /// The units of the roots `roots`; `Err` names why the document is the
    /// root.
    fn rebox(
        &self,
        batch: &mut Batch,
        roots: Vec<Handle>,
        walks: &Walks<'_>,
        cache: &mut RestyleCache,
    ) -> Result<Reboxed, &'static str> {
        let traversal = &self.state.values.traversal;
        traversal.begin_rebuild();
        let (folded, wrappers) = match self.folded(walks, cache) {
            Ok(folded) => folded,
            Err(cause) => {
                traversal.abandon_rebuild();
                return Err(cause);
            }
        };
        let mut pending: Vec<Handle> = roots.iter().map(|r| top(r, walks.restyled)).collect();
        // A rebuilt subtree with a host takes a shadow tree (`shadow_scope`).
        if super::shadow_scope::holds_host(walks.probe, &pending) {
            traversal.abandon_rebuild();
            return Err("shadow_host");
        }
        let mut units: Vec<Unit> = Vec::new();
        while let Some(root) = pending.pop() {
            if units.iter().any(|u| within(&root, &u.handle)) {
                continue;
            }
            match self.unit(batch, &root, walks, cache, (folded.as_ref(), &wrappers)) {
                Ok(unit) => {
                    units.retain(|u| !within(&u.handle, &root));
                    units.push(unit);
                }
                Err(cause) => {
                    if crate::why::enabled() {
                        crate::why::note("paint_path.rebox_widened", || cause.to_string());
                    }
                    match parent(&root).filter(|p| matches!(p.data, NodeData::Element { .. })) {
                        Some(p) => pending.push(top(&p, walks.restyled)),
                        None => {
                            traversal.abandon_rebuild();
                            return Err(cause);
                        }
                    }
                }
            }
        }
        Ok(Reboxed(units))
    }
}

/// The topmost restyled ancestor-or-self of `handle`.
fn top(handle: &Handle, restyled: &HashSet<usize>) -> Handle {
    let mut top = handle.clone();
    let mut at = parent(handle);
    while let Some(node) = at {
        if restyled.contains(&(Rc::as_ptr(&node) as usize)) {
            top = node.clone();
        }
        at = parent(&node);
    }
    top
}

/// `handle` is `root` or lies under it.
fn within(handle: &Handle, root: &Handle) -> bool {
    let mut at = Some(handle.clone());
    while let Some(node) = at {
        if Rc::ptr_eq(&node, root) {
            return true;
        }
        at = parent(&node);
    }
    false
}

/// The parent node of `node` in the source DOM.
fn parent(node: &Handle) -> Option<Handle> {
    let weak = node.parent.take();
    let parent = weak
        .as_ref()
        .and_then(markup5ever_rcdom::WeakHandle::upgrade);
    node.parent.set(weak);
    parent
}

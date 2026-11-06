//! Text blocks that wrapped: not recorded while the flow width changes.
//!
//! A block of text alone whose border box tracks the flow (`width::tracks_flow`)
//! and whose text wrapped in a flow of width `flow` wraps in every flow no
//! wider. Text set from the start edge that does not fit on one line does not
//! fit in less room (CSS Text 3 §5: a line breaks only where its content
//! overflows it), so with the same text the block wraps again, and a record
//! made while the flow width changes would be refused
//! (`width::reusable_range`): such a build does not wrap it for recording.

use super::FlowReuse;
use super::source::Source;
use super::width::Width;
use crate::dom::{ChildrenMark, Element};
use std::cell::RefCell;
use std::collections::HashMap;

/// The wrap of one block at flow width `flow`.
///
/// The entry knows its text by the identity of the block's child list (a
/// changed text is a new list), through a mark that does not keep the list:
/// holding the block's subtree or styles here kept a previous build's styles
/// of every wrapped block alive. Not wrapping a block for recording never
/// changes its layout or paint, only whether a later frame may skip it, so a
/// style change under the same text costs at most that block's record.
#[derive(Clone)]
pub(super) struct Narrow {
    id: u64,
    children: ChildrenMark,
    identity: usize,
    flow: f32,
}

impl Narrow {
    pub(super) fn of(source: &Source, flow: f32) -> Self {
        let children = source.children();
        Self {
            id: source.id(),
            children: children.mark(),
            identity: children.identity(),
            flow,
        }
    }

    /// The block `e` in a flow of width `flow` wraps as it did here.
    fn holds(&self, e: &Element, flow: Option<f32>) -> bool {
        flow.is_some_and(|flow| flow <= self.flow)
            && e.node_id == self.id
            && self.children.is_alive()
            && e.children.identity() == self.identity
    }
}

/// Wraps by node id: those kept between builds (`Store::narrow`) or those
/// seen in one build (`FlowReuse::narrowed`).
#[derive(Default)]
pub(super) struct Wraps(RefCell<HashMap<u64, Narrow>>);

impl Wraps {
    pub(super) fn add(&self, narrow: Narrow) {
        let mut wraps = self.0.take();
        wraps.insert(narrow.id, narrow);
        self.0.replace(wraps);
    }

    /// The flow key changed in more than its width: no wrap holds.
    pub(super) fn reset(&self) {
        self.0.take();
    }
}

impl FlowReuse {
    /// Block `e` of width class `width` may be recorded in a build at the
    /// changed flow width `flow`: its layout does not depend on the width, and
    /// it did not wrap in a flow at least as wide. A wrap that holds is carried
    /// to this build's wraps.
    pub(super) fn width_recordable(&self, e: &Element, width: Width, flow: Option<f32>) -> bool {
        match width {
            Width::Dependent => false,
            Width::Stretch => {
                let kept = self.store.narrow.0.borrow();
                let Some(narrow) = kept.get(&e.node_id).filter(|n| n.holds(e, flow)) else {
                    return true;
                };
                self.narrowed.add(narrow.clone());
                false
            }
            Width::Fixed | Width::Floor => true,
        }
    }

    /// Keep this build's wraps. A build at a changed width visits every block
    /// it may record and replaces them; a settled one adds those of the blocks
    /// it recorded, so the first build at a narrower width already skips them.
    pub(super) fn learn_wraps(&self) {
        let narrowed = self.narrowed.0.take();
        // Карта меняется целиком: свежие переносы заменяют прежние того же блока.
        let mut kept = match self.resized.get() {
            true => Default::default(),
            false => self.store.narrow.0.take(),
        };
        kept.extend(narrowed);
        self.store.narrow.0.replace(kept);
    }
}

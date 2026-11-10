//! A far root-flow block laid out again from its kept layout subtree.
//!
//! When the flow width changes, a block whose layout depends on it (a
//! paragraph that fills the flow and rewraps) has a new size, so neither its
//! exact leaf (`placeholder`) nor its recorded paint (`replay`) fits. Its
//! *elements* are still the same: the build of an unchanged subtree under the
//! same flow key outside the widths that read no width from outside
//! (`BoxConstraints::track_widths`) requests the same layout nodes with the
//! same styles and measure functions. A block far from the screen needs only
//! that layout: it paints nothing, and later blocks need its exact size. So
//! its layout nodes are claimed whole (`Window::request_retained_layout`) and
//! Taffy lays them out at the new width with the measure functions of the
//! build that requested them, kept for the next pass
//! (`Window::keep_retained_measures`): the result is the one a build would
//! give, without building, prepainting or painting the block. Before
//! prepaint the frame checks that every such block is still far from the
//! screen at its new place (`FlowReuse::validate_far`); otherwise the frame
//! is built again with the block built (`Store::refused`).
//!
//! This is Blink's split of the layout cache and paint culling: a clean
//! LayoutObject keeps its children and is laid out again for a new constraint
//! space (`LayoutBox::CachedLayoutResult` misses on the size, the box tree
//! stays), and a fragment outside the cull rect is not painted
//! (`BoxFragmentPainter`, `IntersectsCullRect`).

mod candidate;
mod element;
mod state;

pub(super) use candidate::far_replayable;
pub(super) use element::{Far, FarRecorder};
pub(crate) use state::FrameFar;

use super::{Counters, FlowKey, FlowReuse, Source, scaled};
use crate::dom::Element;
use gpui::{Bounds, InkSink, LayoutId, Pixels, RetainedSubtree, ScaledPixels, Window, point, size};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// How far ink reached outside the layout boxes of a block, per side, in
/// device pixels (`ink_beyond`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Excess {
    top: f32,
    right: f32,
    bottom: f32,
    left: f32,
}

/// A far block's layout subtree with the build inputs it was requested for.
pub(crate) struct FarRecord {
    source: Source,
    key: Rc<FlowKey>,
    start: Counters,
    pub(super) end: Counters,
    layout: RetainedSubtree,
    excess: Excess,
}

impl FarRecord {
    /// The record fits a flow of `key` at counters `start` with subtree `e`.
    pub(super) fn fits(&self, key: &FlowKey, start: Counters, e: &Element) -> bool {
        self.key.same_outside_widths(key) && self.start == start && self.source.same(e)
    }
}

/// Union of a far block's layout boxes and of its hitboxes, if any.
type Placed = (Bounds<ScaledPixels>, Option<Bounds<ScaledPixels>>);

/// A far record in the making: filled by `FarRecorder`, finished after paint.
pub(super) struct PendingFar {
    key: Rc<FlowKey>,
    start: Counters,
    end: Counters,
    layout: RefCell<Option<RetainedSubtree>>,
    /// Prepaint left nothing but hitboxes: the union of the block's layout
    /// boxes and of its hitboxes, if any.
    placed: Cell<Option<Placed>>,
    sink: Rc<InkSink>,
    /// Set when the block painted far from the screen.
    excess: Cell<Option<Excess>>,
}

impl PendingFar {
    pub(super) fn new(key: Rc<FlowKey>, start: Counters, end: Counters) -> Self {
        Self {
            key,
            start,
            end,
            layout: RefCell::new(None),
            placed: Cell::new(None),
            sink: Rc::default(),
            excess: Cell::new(None),
        }
    }

    fn finish(&self, source: Source) -> Option<FarRecord> {
        let excess = self.excess.get()?;
        if self.sink.tainted() {
            return None;
        }
        Some(FarRecord {
            source,
            key: self.key.clone(),
            start: self.start,
            end: self.end,
            layout: self.layout.take()?,
            excess,
        })
    }
}

/// A claimed far block in this frame.
pub(super) struct FarSlot {
    record: Rc<FarRecord>,
    layout: Cell<Option<LayoutId>>,
    /// The flow's widths changed since the last build: the claimed leaves
    /// will be measured, so they make their probes at request time. With the
    /// same widths Taffy's cache serves the subtree and nothing is made.
    prepare: bool,
}

impl FarSlot {
    pub(super) fn new(record: Rc<FarRecord>, prepare: bool) -> Self {
        Self {
            record,
            layout: Cell::new(None),
            prepare,
        }
    }
}

/// Device-pixel bounds of logical `b` at `scale`.
fn device(b: Bounds<Pixels>, scale: f32) -> Bounds<ScaledPixels> {
    Bounds {
        origin: scaled(b.origin, scale),
        size: size(
            ScaledPixels(b.size.width.as_f32() * scale),
            ScaledPixels(b.size.height.as_f32() * scale),
        ),
    }
}

/// How far `ink` reaches outside `boxes` on each side.
fn ink_beyond(boxes: Bounds<ScaledPixels>, ink: Option<Bounds<ScaledPixels>>) -> Excess {
    let Some(ink) = ink else {
        return Excess::default();
    };
    let (b, i) = (boxes, ink);
    Excess {
        top: (b.origin.y.0 - i.origin.y.0).max(0.0),
        left: (b.origin.x.0 - i.origin.x.0).max(0.0),
        bottom: ((i.origin.y.0 + i.size.height.0) - (b.origin.y.0 + b.size.height.0)).max(0.0),
        right: ((i.origin.x.0 + i.size.width.0) - (b.origin.x.0 + b.size.width.0)).max(0.0),
    }
}

/// The layout boxes `boxes`, grown by the ink `excess`, are far from `screen`
/// by the same test as a skipped block's ink (`recorder::far`: a margin of
/// the screen's larger side). The margin covers what the box union cannot
/// show at a new width: glyph ink outside line boxes, which depends on the
/// font, not on the width.
fn clear_of(boxes: Bounds<ScaledPixels>, excess: Excess, screen: Bounds<ScaledPixels>) -> bool {
    let ink = Bounds {
        origin: point(ScaledPixels(-excess.left), ScaledPixels(-excess.top)),
        size: size(
            ScaledPixels(boxes.size.width.0 + excess.left + excess.right),
            ScaledPixels(boxes.size.height.0 + excess.top + excess.bottom),
        ),
    };
    super::recorder::far(Some(ink), boxes.origin, screen)
}

/// The window's content mask in device pixels.
fn screen_of(window: &Window) -> Bounds<ScaledPixels> {
    device(window.content_mask().bounds, window.scale_factor())
}

impl FlowReuse {
    /// Far blocks of this frame that are no longer far at their new place
    /// (or whose layout went stale or was not claimed): they must be built.
    pub(super) fn validate_far(&self, window: &mut Window, root: bool) -> Vec<u64> {
        let Some(mut far) = self.far.take() else {
            return Vec::new();
        };
        let slots = std::mem::take(&mut far.slots);
        self.far.set(Some(far));
        if slots.is_empty() {
            return Vec::new();
        }
        let stale = window.layout_stale();
        let screen = screen_of(window);
        let scale = window.scale_factor();
        let mut failed = Vec::new();
        for slot in &slots {
            let fits = root
                && !stale
                && slot.layout.get().is_some()
                && window
                    .retained_layout_bounds(&slot.record.layout)
                    .is_some_and(|b| clear_of(device(b, scale), slot.record.excess, screen));
            if !fits {
                failed.push(slot.record.source.id());
            }
        }
        failed
    }
}

//! A record in the making: what a recorded block captures in its frame and
//! the replay record it becomes after paint (`PendingReplay::finish`).

use super::{Ranges, ReplayRecord};
use crate::hooks::LineCapture;
use crate::layout::reuse::{Counters, FlowKey, Source};
use gpui::{
    Bounds, ContentMask, InkSinkList, LayoutId, Pixels, Point, RetainedPaint, RetainedPrepaint,
    RetainedSubtree, ScaledPixels, Size,
};
use std::cell::Cell;
use std::rc::Rc;

/// A block's prepaint with its unrounded origin and size.
type PlacedPrepaint = (RetainedPrepaint, Point<Pixels>, Size<Pixels>);

/// A record finished after the frame's paint (`FlowReuse::finish`).
pub(in crate::layout::reuse) struct PendingReplay {
    /// Reads of the outer widths by the build and the measures of the
    /// recording frame.
    pub(in crate::layout::reuse) widths: Rc<Cell<bool>>,
    /// The build and what the frame captured, boxed apart: the element tree
    /// keeps the pending record until the next frame, but all of this goes
    /// to the record at `finish`.
    captured: Cell<Option<Box<Captured>>>,
    /// The block's ink sink in its build's list (`FlowReuse::ink`).
    pub(in crate::layout::reuse) sink: (Rc<InkSinkList>, u32),
}

/// The output of a recorded block in its frame, as it is captured.
pub(in crate::layout::reuse) struct Captured {
    pub(in crate::layout::reuse) key: Option<Rc<FlowKey>>,
    start: Counters,
    end: Counters,
    inert: bool,
    /// The block fills its flow's width (`plain::fills_width`).
    fills: bool,
    paint_last: Option<u64>,
    pub(in crate::layout::reuse) env: Option<(f32, u64)>,
    /// The output moves with the block (`Ranges::movable`) but for its ink.
    pub(in crate::layout::reuse) movable: bool,
    pub(in crate::layout::reuse) layout: Option<(RetainedSubtree, LayoutId)>,
    pub(in crate::layout::reuse) prepaint: Option<PlacedPrepaint>,
    pub(in crate::layout::reuse) paint: Option<RetainedPaint>,
    pub(in crate::layout::reuse) mask: Option<ContentMask<Pixels>>,
    /// The hover its hover boxes painted (`Ranges::hovers`).
    pub(in crate::layout::reuse) hovers: Box<[(gpui::HitboxId, bool)]>,
    pub(in crate::layout::reuse) lines: Option<Rc<LineCapture>>,
}

impl PendingReplay {
    /// `f` over the capture, while the record is not made.
    pub(in crate::layout::reuse) fn capture(&self, f: impl FnOnce(&mut Captured)) {
        if let Some(mut captured) = self.captured.take() {
            f(&mut captured);
            self.captured.set(Some(captured));
        }
    }

    /// `f` over the capture so far, if anything was captured.
    pub(in crate::layout::reuse) fn captured<R>(
        &self,
        f: impl FnOnce(&Captured) -> Option<R>,
    ) -> Option<R> {
        let captured = self.captured.take()?;
        let out = f(&captured);
        self.captured.set(Some(captured));
        out
    }

    pub(in crate::layout::reuse) fn new(
        key: Option<Rc<FlowKey>>,
        (start, end): (Counters, Counters),
        inert: bool,
        widths: Rc<Cell<bool>>,
        fills: bool,
        ink: &Rc<InkSinkList>,
        paint_last: Option<u64>,
    ) -> Self {
        Self {
            widths,
            captured: Cell::new(Some(Box::new(Captured {
                key,
                start,
                end,
                inert: inert && paint_last.is_none(),
                fills,
                paint_last,
                env: None,
                movable: false,
                layout: None,
                prepaint: None,
                paint: None,
                mask: None,
                hovers: Box::default(),
                lines: None,
            }))),
            sink: (ink.clone(), ink.open()),
        }
    }

    /// The finished record of block `source` if its prepaint and paint can
    /// be replayed and it lay near screen `screen` (by its ink, else by its
    /// box): a far block skips the record, its stand-in is cheaper (`recorder::far`).
    ///
    /// The next frame checks a replay before any clip of its ancestors is
    /// known (`ReplaySlot::refusal`): a block drawn under the frame's mask is
    /// checked against it. A box's content drawn under a container's clip
    /// (`Ranges::clipped`) is kept only if its ink lies inside that clip
    /// (`movable`): its copy is clipped by the mask it meets in prepaint
    /// (`Replay`), which draws what a build under that mask draws, so no
    /// clip refuses it after layout. A block of the root flow under another
    /// clip is not recorded, as its replay could only be refused and the
    /// frame built twice (Blink compares the clip node of a cached
    /// subsequence's property tree state; here the copy meets the clip).
    pub(in crate::layout::reuse) fn finish(
        &self,
        source: Source,
        screen: Option<Bounds<ScaledPixels>>,
        frame_mask: Option<Bounds<Pixels>>,
    ) -> Option<ReplayRecord> {
        let (list, at) = &self.sink;
        let (bounds, tainted) = list.with(*at, |sink| (sink.bounds(), sink.tainted()))?;
        if tainted {
            return None;
        }
        let captured = *self.captured.take()?;
        let (layout, _) = captured.layout?;
        let (prepaint, origin, size) = captured.prepaint?;
        let paint = captured.paint?;
        let (scale, serial) = captured.env?;
        let ink = bounds.unwrap_or(Bounds { origin, size }.scale(scale));
        // A far block of the root flow skips the replay record: its
        // stand-in is cheaper (`screen`); a content is recorded everywhere.
        if screen
            .is_some_and(|screen| super::super::recorder::far(Some(ink), Point::default(), screen))
        {
            return None;
        }
        let mask = captured.mask?;
        // A command clipped where it was recorded cannot be restored by a
        // moved copy: only a record whose ink lies inside its mask moves.
        let clip = mask.bounds.scale(scale);
        let movable = captured.movable && bounds.is_none_or(|ink| clip.intersect(&ink) == ink);
        let clipped = frame_mask.is_some_and(|frame| frame != mask.bounds);
        if clipped && !(captured.key.is_none() && movable) {
            return None;
        }
        let lines = captured.lines.filter(|lines| lines.complete())?;
        Some(ReplayRecord {
            fills: captured.fills,
            source,
            key: captured.key,
            widths: self.widths.get(),
            start: captured.start,
            end: captured.end,
            inert: captured.inert,
            paint_last: captured.paint_last,
            ranges: Cell::new(Some(Ranges {
                layout,
                prepaint,
                paint,
                origin,
                size,
                mask,
                scale,
                serial,
                lines,
                movable,
                clipped,
                hovers: captured.hovers,
            })),
        })
    }
}

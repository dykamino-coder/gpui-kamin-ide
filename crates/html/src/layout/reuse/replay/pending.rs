//! A record in the making: what a recorded block captures in its frame and
//! the replay record it becomes after paint (`PendingReplay::finish`).

use super::{ClipFrame, Ranges, ReplayRecord};
use crate::hooks::LineCapture;
use crate::layout::reuse::{Counters, FlowKey, Source};
use gpui::{
    Bounds, ContentMask, InkSinkList, LayoutId, Pixels, Point, RetainedPaint, RetainedPrepaint,
    RetainedSubtree, ScaledPixels, Size,
};
use std::cell::Cell;
use std::rc::Rc;

/// Prepaint блока с его неокруглённым началом и размером.
type PlacedPrepaint = (RetainedPrepaint, Point<Pixels>, Size<Pixels>);

/// Запись, которая завершится после краски кадра (`FlowReuse::finish`).
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
    /// Блок заполняет ширину потока (`plain::fills_width`).
    fills: bool,
    paint_last: Option<u64>,
    pub(in crate::layout::reuse) env: Option<(f32, u64)>,
    /// The output moves with the block (`Ranges::movable`) but for its ink.
    pub(in crate::layout::reuse) movable: bool,
    pub(in crate::layout::reuse) layout: Option<(RetainedSubtree, LayoutId)>,
    pub(in crate::layout::reuse) prepaint: Option<PlacedPrepaint>,
    pub(in crate::layout::reuse) paint: Option<RetainedPaint>,
    pub(in crate::layout::reuse) mask: Option<ContentMask<Pixels>>,
    /// The layout parent at prepaint (`Ranges::container`).
    pub(in crate::layout::reuse) container: Option<ClipFrame>,
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
                container: None,
                lines: None,
            }))),
            sink: (ink.clone(), ink.open()),
        }
    }

    /// Готовая запись блока `source`, если его prepaint и краска переносимы
    /// и он был у экрана `screen` (чернилами, а без них коробкой): дальний
    /// блок пропускает запись пропуска (`recorder::far`), она дешевле повтора.
    ///
    /// The next frame checks a replay before any clip of its ancestors is
    /// known (`ReplaySlot::refusal`): a block drawn under the frame's mask is
    /// checked against it. A box's content drawn under a container's clip
    /// keeps it relative to its layout parent (`ClipFrame`) and replays while
    /// the parent lies the same size where the content moved with it, so the
    /// clip moved with both; where an ancestor's clip changed without them,
    /// the replay is clipped by the mask it meets (`Replay`), so only a
    /// record whose ink lies inside its mask (`movable`) may be clipped. A
    /// block of the root flow under another clip is not recorded, as its
    /// replay could only be refused and the frame built twice (Blink compares
    /// the clip node of a cached subsequence's property tree state).
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
        let container = match frame_mask {
            Some(frame) if frame != mask.bounds => {
                let inside = frame.intersect(&mask.bounds) == mask.bounds;
                let content = captured.key.is_none();
                Some(
                    captured
                        .container
                        .filter(|_| content && movable && inside)?,
                )
            }
            _ => None,
        };
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
                container,
            })),
        })
    }
}

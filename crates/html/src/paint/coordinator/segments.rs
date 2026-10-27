//! Допуск CSS-узлов, выбор overflow-цепочки CB и собственные записи внепоточных
//! корней в одной сегментной записи источника.

use gpui::{
    App, Bounds, PaintPlacement, PaintPlan, PaintRecordError, PaintSegment, PaintSegmentClips,
    PaintSegmentRecords, Pixels, Window, point, px,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use crate::layout::fragment::decoration::Decoration;
use crate::layout::fragment::paint_source::{BareRecords, Nested, SourceSlices};
mod boxes;
mod capture;
pub(crate) use boxes::{box_painter, decoration, decorations};
mod shift;
use capture::own_capture;

struct Source {
    slices: SourceSlices,
    painting: bool,
    bare: BareRecords,
    /// CB clip chains by (active capture, node).
    clips: RefCell<HashMap<(u64, u64), PaintSegmentClips>>,
    failure: Cell<Option<PaintRecordError>>,
}

impl Source {
    fn new(slices: SourceSlices, painting: bool) -> Rc<Self> {
        Rc::new(Self {
            slices,
            painting,
            bare: BareRecords::default(),
            clips: RefCell::default(),
            failure: Cell::default(),
        })
    }

    fn fail(&self, error: PaintRecordError) {
        self.failure.set(Some(error));
    }

    fn remember(&self, key: (u64, u64), clips: PaintSegmentClips) {
        self.clips.borrow_mut().insert(key, clips);
    }

    /// The clip chain selected by a containing block. A CB outside the source
    /// subtree (the fragmentation host, the initial containing block) shares the
    /// source root's chain.
    fn cb_clips(&self, key: u64, cb: u64) -> Option<PaintSegmentClips> {
        let clips = self.clips.borrow();
        clips
            .get(&(key, cb))
            .or_else(|| (key == 0).then(|| clips.get(&(0, 0))).flatten())
            .cloned()
    }

    fn family(&self, id: u64) -> u64 {
        self.slices.families.get(&id).copied().unwrap_or(id)
    }

    /// The capture-local admission of `id`'s family inside the active capture.
    fn own(&self, id: u64, key: u64) -> Option<Rc<[gpui::PaintSegmentSlice]>> {
        let nested = self.slices.nested.get(&key).filter(|_| key != 0)?;
        nested.families.get(&self.family(id)).cloned()
    }

    /// Decorations of `id` inside the active capture, else of the source record,
    /// moved with their root's final placement.
    fn boxes(&self, id: u64, key: u64, by: gpui::Point<Pixels>) -> Option<Rc<[Decoration]>> {
        let boxes = match self.slices.nested.get(&key).filter(|_| key != 0) {
            Some(nested) => nested.boxes.get(&id).cloned(),
            None => self.slices.boxes.get(&id).cloned(),
        };
        boxes.map(|boxes| shift::boxes(boxes, by))
    }
}

struct CurrentNode(u64);
/// Family root whose own capture is active.
struct Capture(u64);

fn capture_key(window: &Window) -> u64 {
    window.render_context::<Capture>().map_or(0, |c| c.0)
}

pub(crate) fn record(
    extent: Bounds<Pixels>,
    segments: Rc<[PaintSegment]>,
    slices: SourceSlices,
    window: &mut Window,
    cx: &mut App,
    paint: impl FnOnce(&mut Window, &mut App),
) -> Result<(PaintSegmentRecords, BareRecords), PaintRecordError> {
    let source = Source::new(slices, true);
    let (_, records) = window.record_paint_segments(extent, segments, cx, |window, cx| {
        match window.paint_segment_clips() {
            Ok(clips) => source.remember((0, 0), clips),
            Err(error) => source.fail(error),
        }
        window.with_render_context(Some(source.clone()), |window| paint(window, cx));
    })?;
    if let Some(error) = source.failure.take() {
        return Err(error);
    }
    Ok((records, source.bare.clone()))
}

pub(crate) fn prepaint(
    extent: Bounds<Pixels>,
    segments: Rc<[PaintSegment]>,
    slices: SourceSlices,
    window: &mut Window,
    cx: &mut App,
    prepaint: impl FnOnce(&mut Window, &mut App),
) -> Result<(), PaintRecordError> {
    let source = Source::new(slices, false);
    window.with_prepaint_segments(extent, segments, cx, |window, cx| {
        match window.paint_segment_clips() {
            Ok(clips) => source.remember((0, 0), clips),
            Err(error) => source.fail(error),
        }
        let traversal = Rc::new(gpui::PrepaintTraversal {
            enter_children: Rc::new(remember_clips),
        });
        window.with_render_context(Some(source.clone()), |window| {
            window.with_render_context(Some(traversal), |window| prepaint(window, cx));
        });
    })?;
    source.failure.take().map_or(Ok(()), Err)
}

pub(crate) fn node(
    id: u64,
    cb: Option<u64>,
    placed: gpui::Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
    paint: impl FnOnce(&mut Window, &mut App),
) {
    let Some(source) = window.render_context::<Source>() else {
        paint(window, cx);
        return;
    };
    // A continued root placed elsewhere than its admission's layout (a late
    // static position) carries its family's admission along.
    match source.slices.origins.get(&id).map(|origin| placed - *origin) {
        Some(by) if by != gpui::Point::default() => {
            window.with_render_context(Some(Rc::new(shift::Shift(by))), |window| {
                admitted(source, id, cb, window, cx, paint)
            })
        }
        _ => admitted(source, id, cb, window, cx, paint),
    }
}

fn admitted(
    source: Rc<Source>,
    id: u64,
    cb: Option<u64>,
    window: &mut Window,
    cx: &mut App,
    paint: impl FnOnce(&mut Window, &mut App),
) {
    let key = capture_key(window);
    let by = shift::current(window);
    if source.painting
        && key == 0
        && let Some(nested) = source.slices.nested.get(&id).cloned()
    {
        let nested = shift::nested(nested, by);
        own_capture(&source, id, cb, nested, key, window, cx, paint);
        return;
    }
    let slices = match source.own(id, key) {
        // A capture's own admission was shifted with its root already.
        Some(slices) => slices,
        None if key != 0 => Rc::from([]),
        None => shift::slices(
            source
                .slices
                .nodes
                .get(&id)
                .cloned()
                .unwrap_or_else(|| Rc::from([])),
            by,
        ),
    };
    let paint = |window: &mut Window| {
        window.with_render_context(Some(Rc::new(CurrentNode(id))), |window| paint(window, cx));
    };
    let clips = cb.map(|cb| source.cb_clips(key, cb));
    let result = window.with_paint_segment_slices(slices, |window| match clips {
        Some(Some(clips)) => window.with_paint_segment_clips(&clips, paint),
        // Inside an own capture the CB may lie outside: its clip belongs to
        // the enclosing record or the legacy copy that replays this one.
        Some(None) if key != 0 => {
            paint(window);
            Ok(())
        }
        Some(None) => {
            Err(PaintRecordError::InvalidGeometry)
        }
        None => {
            paint(window);
            Ok(())
        }
    });
    if let Err(error) = result.and_then(|result| result) {
        source.fail(error);
    }
}

pub(super) fn remember_clips(window: &Window) {
    let (Some(source), Some(node)) = (
        window.render_context::<Source>(),
        window.render_context::<CurrentNode>(),
    ) else {
        return;
    };
    let key = (capture_key(window), node.0);
    if source.clips.borrow().contains_key(&key) {
        return;
    }
    match window.paint_segment_clips() {
        Ok(clips) => source.remember(key, clips),
        Err(error) => source.fail(error),
    }
}

//! Допуск CSS-узлов и выбор overflow-цепочки CB в одной сегментной записи.

use gpui::{
    App, Bounds, PaintRecordError, PaintSegment, PaintSegmentClips, PaintSegmentRecords, Pixels,
    Window,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use crate::layout::fragment::decoration::Decoration;
use crate::layout::fragment::paint_source::SourceSlices;

struct Source {
    slices: SourceSlices,
    clips: RefCell<HashMap<u64, PaintSegmentClips>>,
    failure: Cell<Option<PaintRecordError>>,
}

impl Source {
    fn remember(&self, id: u64, clips: PaintSegmentClips) {
        self.clips.borrow_mut().insert(id, clips);
    }
}

struct CurrentNode(u64);

/// Потребитель таблиц P18 остаётся в CSS-слоте своего исходного узла.
/// Слот вызывается при capture; immutable playback его не повторяет.
pub(crate) fn slot(id: u64, window: &mut Window, cx: &mut App) {
    let callback = window
        .render_context::<Source>()
        .and_then(|source| source.slices.slots.get(&id).cloned());
    if let Some(callback) = callback
        && let Err(error) = callback(window, cx)
        && let Some(source) = window.render_context::<Source>()
    {
        source.failure.set(Some(error));
    }
}

pub(crate) fn record(
    extent: Bounds<Pixels>,
    segments: Rc<[PaintSegment]>,
    slices: SourceSlices,
    window: &mut Window,
    cx: &mut App,
    paint: impl FnOnce(&mut Window, &mut App),
) -> Result<PaintSegmentRecords, PaintRecordError> {
    let source = Rc::new(Source {
        slices,
        clips: RefCell::default(),
        failure: Cell::default(),
    });
    let (_, records) = window.record_paint_segments(extent, segments, cx, |window, cx| {
        match window.paint_segment_clips() {
            Ok(clips) => {
                source.remember(0, clips);
            }
            Err(error) => {
                source.failure.set(Some(error));
            }
        }
        window.with_render_context(Some(source.clone()), |window| {
            paint(window, cx);
        });
    })?;
    if let Some(error) = source.failure.take() {
        return Err(error);
    }
    Ok(records)
}

pub(crate) fn prepaint(
    extent: Bounds<Pixels>,
    segments: Rc<[PaintSegment]>,
    slices: SourceSlices,
    window: &mut Window,
    cx: &mut App,
    prepaint: impl FnOnce(&mut Window, &mut App),
) -> Result<(), PaintRecordError> {
    let source = Rc::new(Source {
        slices,
        clips: RefCell::default(),
        failure: Cell::default(),
    });
    window.with_prepaint_segments(extent, segments, cx, |window, cx| {
        match window.paint_segment_clips() {
            Ok(clips) => {
                source.remember(0, clips);
            }
            Err(error) => {
                source.failure.set(Some(error));
            }
        }
        let traversal = Rc::new(gpui::PrepaintTraversal {
            enter_children: Rc::new(remember_clips),
        });
        window.with_render_context(Some(source.clone()), |window| {
            window.with_render_context(Some(traversal), |window| prepaint(window, cx));
        });
    })?;
    let failure = source.failure.take();
    failure.map_or(Ok(()), Err)
}

pub(crate) fn node(id: u64, cb: Option<u64>, window: &mut Window, paint: impl FnOnce(&mut Window)) {
    let Some(source) = window.render_context::<Source>() else {
        paint(window);
        return;
    };
    let slices = source
        .slices
        .nodes
        .get(&id)
        .cloned()
        .unwrap_or_else(|| Rc::from([]));
    let paint = |window: &mut Window| {
        window.with_render_context(Some(Rc::new(CurrentNode(id))), paint);
    };
    let clips = cb.map(|cb| source.clips.borrow().get(&cb).cloned());
    let result = window.with_paint_segment_slices(slices, |window| match clips {
        Some(clips) => match clips {
            Some(clips) => window.with_paint_segment_clips(&clips, paint),
            None => Err(PaintRecordError::InvalidGeometry),
        },
        None => {
            paint(window);
            Ok(())
        }
    });
    if let Err(error) = result.and_then(|result| result) {
        source.failure.set(Some(error));
    }
}

pub(super) fn remember_clips(window: &Window) {
    let (Some(source), Some(node)) = (
        window.render_context::<Source>(),
        window.render_context::<CurrentNode>(),
    ) else {
        return;
    };
    if source.clips.borrow().contains_key(&node.0) {
        return;
    }
    match window.paint_segment_clips() {
        Ok(clips) => {
            source.remember(node.0, clips);
        }
        Err(error) => {
            source.failure.set(Some(error));
        }
    }
}

pub(crate) fn decorations(id: u64, window: &Window) -> Rc<[Decoration]> {
    window
        .render_context::<Source>()
        .and_then(|source| source.slices.boxes.get(&id).cloned())
        .unwrap_or_else(|| Rc::from([]))
}

pub(crate) fn decoration(
    geometry: Decoration,
    window: &mut Window,
    paint: impl FnOnce(&mut Window),
) {
    let result = window.with_paint_segment_slices(Rc::from([geometry.slice]), paint);
    if let Err(error) = result {
        if let Some(source) = window.render_context::<Source>() {
            source.failure.set(Some(error));
        }
    }
}

pub(crate) fn box_painter(id: u64) -> gpui::BoxPainter {
    gpui::BoxPainter(Rc::new(move |style, _, window, cx| {
        paint_box(id, style, window, cx)
    }))
}

fn paint_box(id: u64, style: &gpui::Style, window: &mut Window, cx: &mut App) -> bool {
    let Some(source) = window.render_context::<Source>() else {
        return false;
    };
    // Только узлы с явным вводом заменяют свою краску; служебные native-узлы
    // продолжают общий painter без CSS-эвристик в GPUI.
    let Some(boxes) = source.slices.boxes.get(&id) else {
        return false;
    };
    for geometry in boxes.iter() {
        decoration(*geometry, window, |window| {
            style.paint_box(geometry.bounds, window, cx)
        });
    }
    true
}

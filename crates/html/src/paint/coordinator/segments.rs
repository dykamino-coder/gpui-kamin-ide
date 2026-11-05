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
pub(crate) use boxes::{box_painter, decoration_async, decorations};
mod shift;
#[cfg(test)]
use capture::own_capture;
pub(super) use capture::remember_clips;

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

/// A root presented per fragment under its own transform (`SourceSlices::
/// presented`): its transformed element paints untransformed in the source.
pub(crate) fn presented_root(node: u64, window: &Window) -> bool {
    let Some(source) = window.render_context::<Source>() else {
        return false;
    };
    source.slices.presented.contains(&node) && (!source.painting || capture_key(window) == node)
}

fn capture_key(window: &Window) -> u64 {
    window.render_context::<Capture>().map_or(0, |c| c.0)
}

pub(crate) async fn outside_source_async<R>(
    window: &mut Window,
    f: impl AsyncFnOnce(&mut Window) -> R,
) -> R {
    window
        .with_render_context_async::<Source, R>(None, async |window| {
            window
                .with_render_context_async::<CurrentNode, R>(None, f)
                .await
        })
        .await
}

mod admission;
#[cfg(test)]
pub(crate) use admission::node;
pub(crate) use admission::node_async;

mod traversal;
pub(crate) use traversal::{prepaint_async, record_async};

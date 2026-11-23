//! Владелец прокручиваемого переполнения HTML (CSS Overflow 3 §2.2): GPUI измеряет только
//! коробки непосредственных детей, поэтому `Frame` собирает за кадр коробки потомков, текста,
//! сеточных дорожек и областей snap, а прозрачный прокси переполнения проецирует их объединение
//! в нативный диапазон прокрутки. Здесь же решения о полосах, их gutter и обрезке декораций.
pub(crate) mod classic;
mod exact_probe;
pub(crate) use exact_probe::{ShiftLink, StickyShift};
mod geometry;
mod grid;
mod snap;
use crate::dom::Element;
use crate::layout::probe::Sink;
use crate::style::computed::Computed;
use geometry::{Clip, Rect};
use gpui::{Bounds, Pixels, Size, px, size};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

#[derive(Clone)]
struct BoxRecord {
    id: u64,
    rect: Rect,
    clip: Clip,
    snap: Option<snap::Area>,
}

/// What layouts learned of a scrollport's bars: whether one measured its
/// range (until then the viewport guesses its bar), why layout does not
/// decide its bars (`classic::layout_axes`), and the inputs whose change
/// (bars layout did not decide) lays the roots out again in the same draw,
/// one per layout again, as one bar can bring the other. decided: the axes
/// whose bar layout decides; the measured range only checks those.
#[derive(Default)]
struct Settling {
    settled: Cell<bool>,
    refusal: Cell<Option<&'static str>>,
    decided: Cell<[bool; 2]>,
    inputs: [gpui::LayoutInputId; 3],
}

#[derive(Clone)]
pub struct Frame {
    boxes: Sink<BoxRecord>,
    text_boxes: Rc<RefCell<HashMap<u64, Rect>>>,
    grid_bounds: Sink<Option<Rect>>,
    extent: Rc<Cell<Size<Pixels>>>,
    marker: Sink<()>,
    root: Rc<Cell<Option<u64>>>,
    viewport: Rc<Cell<bool>>,
    snapped: Rc<Cell<bool>>,
    bars: Rc<Cell<[bool; 2]>>,
    reverse: Rc<Cell<[bool; 2]>>,
    handle: Rc<RefCell<Option<gpui::ScrollHandle>>>,
    reserved: Rc<Cell<[f32; 2]>>,
    range: Rc<Cell<[f32; 2]>>,
    insets: Rc<Cell<[f32; 4]>>,
    native_reserved: Rc<Cell<[f32; 2]>>,
    decoration_clip: Rc<Cell<Option<gpui::ContentMask<Pixels>>>>,
    settling: Rc<Settling>,
}
impl Default for Frame {
    fn default() -> Self {
        Self {
            boxes: Sink::default(),
            text_boxes: Rc::default(),
            grid_bounds: Sink::default(),
            extent: Rc::new(Cell::new(size(px(0.0), px(0.0)))),
            marker: Sink::default(),
            root: Rc::new(Cell::new(None)),
            viewport: Rc::default(),
            snapped: Rc::default(),
            bars: Rc::default(),
            reverse: Rc::default(),
            handle: Rc::default(),
            reserved: Rc::default(),
            range: Rc::default(),
            insets: Rc::default(),
            native_reserved: Rc::default(),
            decoration_clip: Rc::default(),
            settling: Rc::default(),
        }
    }
}
impl Frame {
    pub(super) fn set_handle(&self, handle: gpui::ScrollHandle) {
        *self.handle.borrow_mut() = Some(handle);
    }
    pub(super) fn parent_for(frame: Option<&Self>, id: u64) -> Option<Self> {
        frame.filter(|f| f.root.get() != Some(id)).cloned()
    }
    pub(super) fn record_border_box(
        &self,
        id: u64,
        c: &Computed,
        mut b: Bounds<Pixels>,
        sticky: Option<&exact_probe::StickyShift>,
    ) {
        b.origin -= exact_probe::sticky_shift(sticky);
        self.boxes.push(BoxRecord {
            id,
            rect: [
                f32::from(b.left()),
                f32::from(b.top()),
                f32::from(b.right()),
                f32::from(b.bottom()),
            ],
            clip: Clip::new(c),
            snap: None,
        });
    }

    pub(crate) fn layout_inputs(&self) -> [gpui::LayoutInputId; 3] {
        self.settling.inputs
    }
    pub(crate) fn bars(&self) -> [bool; 2] {
        self.bars.get()
    }
    pub(super) fn insets(&self) -> [f32; 4] {
        self.insets.get()
    }
    pub(super) fn range(&self) -> [f32; 2] {
        self.range.get()
    }
    pub(super) fn set_decoration_clip(&self, clip: gpui::ContentMask<Pixels>) {
        self.decoration_clip.set(Some(clip));
    }
    pub(super) fn begin(&self, root: Option<&Element>) -> Option<Self> {
        self.boxes.take();
        self.text_boxes.borrow_mut().clear();
        self.grid_bounds.take();
        self.marker.take();
        self.root.set(root.map(|e| e.node_id));
        self.viewport
            .set(root.is_some_and(|e| e.tag == "html-viewport"));
        root.map(|_| self.clone())
    }
}

mod probes;
mod projection;
mod settle;
pub(crate) use probes::{decoration_clip, marker, probe, record_text, viewport_decoration};
pub(crate) use projection::{project, projection};

#[cfg(test)]
mod tests;

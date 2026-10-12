//! HTML scrollable-overflow owner; GPUI measures only immediate child bounds.
//! A transparent extent proxy projects CSS Overflow 3 §2.2 into that native range.
pub(crate) mod classic;
mod exact_probe;
pub(crate) use exact_probe::sticky_geometry;
mod geometry;
mod snap;
use crate::dom::Element;
use crate::style::computed::Computed;
use geometry::{Clip, Rect};
use gpui::{Bounds, Pixels, Size, px, size};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

#[derive(Clone)]
pub struct Frame {
    boxes: Rc<RefCell<HashMap<u64, (Rect, Clip)>>>,
    text_boxes: Rc<RefCell<HashMap<u64, Rect>>>,
    extent: Rc<Cell<Size<Pixels>>>,
    marker: Rc<Cell<bool>>,
    root: Rc<Cell<Option<u64>>>,
    viewport: Rc<Cell<bool>>,
    snap_areas: Rc<RefCell<HashMap<u64, snap::Area>>>,
    snapped: Rc<Cell<bool>>,
    bars: Rc<Cell<[bool; 2]>>,
    reserved: Rc<Cell<[f32; 2]>>,
    range: Rc<Cell<[f32; 2]>>,
    insets: Rc<Cell<[f32; 4]>>,
    native_reserved: Rc<Cell<[f32; 2]>>,
    decoration_clip: Rc<Cell<Option<gpui::ContentMask<Pixels>>>>,
}
impl Default for Frame {
    fn default() -> Self {
        Self {
            boxes: Rc::default(),
            text_boxes: Rc::default(),
            extent: Rc::new(Cell::new(size(px(0.0), px(0.0)))),
            marker: Rc::new(Cell::new(false)),
            root: Rc::new(Cell::new(None)),
            viewport: Rc::default(),
            snap_areas: Rc::default(),
            snapped: Rc::default(),
            bars: Rc::default(),
            reserved: Rc::default(),
            range: Rc::default(),
            insets: Rc::default(),
            native_reserved: Rc::default(),
            decoration_clip: Rc::default(),
        }
    }
}
thread_local! { static CURRENT: RefCell<Option<Frame>> = const { RefCell::new(None) }; }
struct Restore(Option<Frame>);
impl Drop for Restore {
    fn drop(&mut self) {
        CURRENT.with(|slot| *slot.borrow_mut() = self.0.take());
    }
}
impl Frame {
    pub(super) fn parent_for(id: u64) -> Option<Self> {
        CURRENT
            .with(|slot| slot.borrow().clone())
            .filter(|f| f.root.get() != Some(id))
    }
    pub(super) fn record_border_box(&self, id: u64, c: &Computed, mut b: Bounds<Pixels>) {
        b.origin -= exact_probe::sticky_shift();
        self.boxes.borrow_mut().insert(
            id,
            (
                [
                    f32::from(b.left()),
                    f32::from(b.top()),
                    f32::from(b.right()),
                    f32::from(b.bottom()),
                ],
                Clip::new(c),
            ),
        );
    }

    pub(super) fn bars(&self) -> [bool; 2] {
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
    pub(super) fn within<R>(&self, root: Option<&Element>, f: impl FnOnce() -> R) -> R {
        self.boxes.borrow_mut().clear();
        self.text_boxes.borrow_mut().clear();
        self.snap_areas.borrow_mut().clear();
        self.marker.set(false);
        self.root.set(root.map(|e| e.node_id));
        self.viewport
            .set(root.is_some_and(|e| e.tag == "html-viewport"));
        let active = root.map(|_| self.clone());
        let restore = Restore(CURRENT.with(|slot| slot.replace(active)));
        let result = f();
        drop(restore);
        result
    }
}

mod probes;
mod settle;
pub(crate) use probes::{
    decoration_clip, marker, probe, record_text, text_active, viewport_decoration,
};

//! HTML scrollable-overflow owner; GPUI measures only immediate child bounds.
//! A transparent extent proxy projects CSS Overflow 3 §2.2 into that native range.
mod geometry;
mod snap;
use crate::dom::{Element, Node};
use crate::style::computed::Computed;
use geometry::{Clip, Rect, sides, union};
use gpui::{AnyElement, Bounds, IntoElement, Pixels, ScrollHandle, Size, Styled, px, size};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

#[derive(Clone)]
pub struct Frame {
    boxes: Rc<RefCell<HashMap<u64, (Rect, Clip)>>>,
    extent: Rc<Cell<Size<Pixels>>>,
    marker: Rc<Cell<bool>>,
    root: Rc<Cell<Option<u64>>>,
    snap_areas: Rc<RefCell<HashMap<u64, snap::Area>>>,
    snapped: Rc<Cell<bool>>,
    decoration_clip: Rc<Cell<Option<gpui::ContentMask<Pixels>>>>,
}
impl Default for Frame {
    fn default() -> Self {
        Self {
            boxes: Rc::default(),
            extent: Rc::new(Cell::new(size(px(0.0), px(0.0)))),
            marker: Rc::new(Cell::new(false)),
            root: Rc::new(Cell::new(None)),
            snap_areas: Rc::default(),
            snapped: Rc::default(),
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
    pub(super) fn set_decoration_clip(&self, clip: gpui::ContentMask<Pixels>) {
        self.decoration_clip.set(Some(clip));
    }
    pub(super) fn within<R>(&self, root: Option<&Element>, f: impl FnOnce() -> R) -> R {
        self.boxes.borrow_mut().clear();
        self.snap_areas.borrow_mut().clear();
        self.marker.set(false);
        self.root.set(root.map(|e| e.node_id));
        let active = root.map(|_| self.clone());
        let restore = Restore(CURRENT.with(|slot| slot.replace(active)));
        let result = f();
        drop(restore);
        result
    }
    pub(super) fn settle(&self, tree: &Element, c: &Computed, handle: &ScrollHandle) -> bool {
        if !self.marker.get() {
            return false;
        }
        let boxes = self.boxes.borrow();
        fn area(e: &Element, boxes: &HashMap<u64, (Rect, Clip)>) -> Option<Rect> {
            let descendants = e
                .children
                .iter()
                .filter_map(|node| match node {
                    Node::Element(child) => area(child, boxes),
                    _ => None,
                })
                .reduce(union);
            let Some(&(own, clip)) = boxes.get(&e.node_id) else {
                return descendants;
            };
            Some(
                descendants
                    .and_then(|r| clip.intersect(own, r))
                    .map_or(own, |r| union(own, r)),
            )
        }
        let bounds = handle.bounds();
        let border = sides(c.borders(), 0.0);
        let padding = sides(c.padding, f32::from(bounds.size.width));
        let origin = [
            f32::from(bounds.left()) + border[0],
            f32::from(bounds.top()) + border[1],
        ];
        let view = [
            f32::from(bounds.size.width) - border[0] - border[2],
            f32::from(bounds.size.height) - border[1] - border[3],
        ];
        let offset = handle.offset();
        let mut end = [origin[0] + view[0], origin[1] + view[1]];
        for node in &tree.children {
            if let Node::Element(e) = node
                && let Some(rect) = area(e, &boxes)
            {
                end[0] = end[0].max(rect[2] - f32::from(offset.x) + padding[2]);
                end[1] = end[1].max(rect[3] - f32::from(offset.y) + padding[3]);
            }
        }
        // The proxy is child zero. Native children also cover text-only runs
        // and CSS boxes whose own nested scrollport owns their descendants.
        if let Some(marker) = handle.bounds_for_item(0) {
            for index in 1..handle.children_count() {
                if let Some(child) = handle.bounds_for_item(index) {
                    end[0] = end[0]
                        .max(origin[0] + f32::from(child.right() - marker.left()) + padding[2]);
                    end[1] = end[1]
                        .max(origin[1] + f32::from(child.bottom() - marker.top()) + padding[3]);
                }
            }
        }
        // GPUI adds padding and subtracts the outer border box. Convert the
        // CSS padding-box overflow extent into that content-size convention.
        let extent = size(
            px((end[0] - origin[0] + border[0] + border[2] - padding[0] - padding[2]).max(0.0)),
            px((end[1] - origin[1] + border[1] + border[3] - padding[1] - padding[3]).max(0.0)),
        );
        let changed = self.extent.replace(extent) != extent;
        if changed {
            return true;
        }
        if c.snap.axis != 0 && !self.snapped.get() && !self.snap_areas.borrow().is_empty() {
            self.snapped.set(true);
            let mut areas: Vec<_> = self
                .snap_areas
                .borrow()
                .iter()
                .map(|(&id, &area)| (id, area))
                .collect();
            areas.sort_by_key(|(id, _)| *id);
            return snap::initial(c, areas.into_iter().map(|(_, area)| area), handle);
        }
        false
    }
}

pub(crate) fn decoration_clip(id: u64) -> Option<Rc<Cell<Option<gpui::ContentMask<Pixels>>>>> {
    let frame = CURRENT.with(|slot| slot.borrow().clone())?;
    (frame.root.get() == Some(id)).then(|| frame.decoration_clip.clone())
}

pub(crate) fn marker(id: u64) -> Option<AnyElement> {
    let frame = CURRENT.with(|slot| slot.borrow().clone())?;
    if frame.root.get() != Some(id) {
        return None;
    }
    let extent = frame.extent.get();
    Some(
        gpui::canvas(move |_, _, _| frame.marker.set(true), |_, _, _, _| {})
            .absolute()
            .top_0()
            .left_0()
            .w(extent.width)
            .h(extent.height)
            .into_any_element(),
    )
}

pub(crate) fn probe(id: u64, c: &Computed) -> Option<AnyElement> {
    let frame = CURRENT.with(|slot| slot.borrow().clone())?;
    if frame.root.get() == Some(id) {
        return None;
    }
    let clip = Clip::new(c);
    let snap = c.snap;
    let vertical = c.vertical == Some(true);
    let reverse = [
        if vertical {
            c.vertical_rl == Some(true)
        } else {
            c.rtl == Some(true)
        },
        vertical && c.rtl == Some(true),
    ];
    Some(
        gpui::canvas(
            move |b: Bounds<Pixels>, _, _| {
                let border = clip.border();
                let rect = [
                    f32::from(b.left()) - border[0],
                    f32::from(b.top()) - border[1],
                    f32::from(b.right()) + border[2],
                    f32::from(b.bottom()) + border[3],
                ];
                frame.boxes.borrow_mut().insert(id, (rect, clip));
                if snap.align != [0; 2] {
                    frame.snap_areas.borrow_mut().insert(
                        id,
                        snap::Area {
                            rect,
                            snap,
                            vertical,
                            reverse,
                        },
                    );
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .into_any_element(),
    )
}

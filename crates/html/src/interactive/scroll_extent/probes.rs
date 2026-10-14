//! Measured CSS boxes and extent proxies belonging to the current HTML scrollport.
use super::{CURRENT, Clip, snap};
use crate::style::computed::Computed;
use gpui::{AnyElement, Bounds, IntoElement, Pixels, Styled};
use std::{cell::Cell, rc::Rc};

pub(crate) fn viewport_decoration()
-> Option<([f32; 4], Rc<Cell<Option<gpui::ContentMask<Pixels>>>>)> {
    let frame = CURRENT.with(|slot| slot.borrow().clone())?;
    frame
        .viewport
        .get()
        .then(|| (frame.insets.get(), frame.decoration_clip.clone()))
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
        return super::grid::probe(frame, c);
    }
    let clip = Clip::new(c);
    let snap = c.snap;
    let vertical = c.vertical == Some(true);
    let reverse = super::super::scrollbar_gutter::reversed(c);
    Some(
        super::exact_probe::ExactProbe::new(move |b: Bounds<Pixels>, _snapped: Bounds<Pixels>| {
            let border = clip.border();
            let rect = [
                f32::from(b.left()) - border[0],
                f32::from(b.top()) - border[1],
                f32::from(b.right()) + border[2],
                f32::from(b.bottom()) + border[3],
            ];
            frame.boxes.borrow_mut().insert(id, (rect, clip));
            if snap.align != [0; 2] {
                // Inflate CSS geometry before device snapping. Independently
                // snapped padding edges fabricate 0.8px on a 50px border at 1.25x.
                let mut snap_bounds = b;
                snap_bounds.origin += super::exact_probe::sticky_shift();
                frame.snap_areas.borrow_mut().insert(
                    id,
                    snap::Area {
                        rect: [
                            f32::from(snap_bounds.left()) - border[0],
                            f32::from(snap_bounds.top()) - border[1],
                            f32::from(snap_bounds.right()) + border[2],
                            f32::from(snap_bounds.bottom()) + border[3],
                        ],
                        snap,
                        vertical,
                        reverse,
                    },
                );
            }
        })
        .into_any_element(),
    )
}

pub(crate) fn text_active() -> bool {
    CURRENT.with(|slot| slot.borrow().is_some())
}
/// Line boxes contribute to scrollable overflow, never text-decoration ink.
pub(crate) fn record_text(id: u64, mut b: Bounds<Pixels>) {
    let Some(frame) = CURRENT.with(|slot| slot.borrow().clone()) else {
        return;
    };
    b.origin -= super::exact_probe::sticky_shift();
    let rect = [
        f32::from(b.left()),
        f32::from(b.top()),
        f32::from(b.right()),
        f32::from(b.bottom()),
    ];
    frame
        .text_boxes
        .borrow_mut()
        .entry(id)
        .and_modify(|r| *r = super::geometry::union(*r, rect))
        .or_insert(rect);
}

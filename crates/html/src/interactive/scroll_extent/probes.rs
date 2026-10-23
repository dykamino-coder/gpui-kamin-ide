//! Щупы текущего HTML-scrollport: прозрачные элементы и записи, через которые раскладка
//! сообщает `Frame` измеренные коробки потомков, строки текста, области scroll-snap и маркер
//! прокси переполнения, а покраска получает обрезку декораций вьюпорта и корня прокрутки.
//! В переполнение входят line box'ы, но не чернила `text-decoration`.

use super::{Clip, Frame, snap};
use crate::style::computed::Computed;
use gpui::{AnyElement, Bounds, Pixels};
use std::{cell::Cell, rc::Rc};

// The viewport probe pairs its four insets with the shared decoration clip.
#[allow(clippy::type_complexity)]
pub(crate) fn viewport_decoration(
    frame: Option<&Frame>,
) -> Option<([f32; 4], Rc<Cell<Option<gpui::ContentMask<Pixels>>>>)> {
    let frame = frame?.clone();
    frame
        .viewport
        .get()
        .then(|| (frame.insets.get(), frame.decoration_clip.clone()))
}

pub(crate) fn decoration_clip(
    frame: Option<&Frame>,
    id: u64,
) -> Option<Rc<Cell<Option<gpui::ContentMask<Pixels>>>>> {
    let frame = frame?.clone();
    (frame.root.get() == Some(id)).then(|| frame.decoration_clip.clone())
}

pub(crate) fn marker(frame: Option<&Frame>, id: u64) -> Option<AnyElement> {
    let frame = frame?.clone();
    if frame.root.get() != Some(id) {
        return None;
    }
    let extent = frame.extent.get();
    let mut style = crate::layout::probe::full_box();
    style.size = gpui::size(extent.width.into(), extent.height.into());
    Some(frame.marker.record_styled(style, |_, _| ()))
}

pub(crate) fn probe(
    frame: Option<&Frame>,
    id: u64,
    c: &Computed,
    sticky: Option<&super::exact_probe::StickyShift>,
) -> Option<AnyElement> {
    let frame = frame?.clone();
    if frame.root.get() == Some(id) {
        return super::grid::probe(frame, c, sticky.cloned());
    }
    let clip = Clip::new(c);
    let snap = c.snap;
    let vertical = c.wm().vertical();
    let reverse = super::super::scrollbar_gutter::reversed(c);
    let link = sticky.cloned();
    Some(frame.boxes.record_with(move |g, window| {
        let half = gpui::px(0.5 / window.scale_factor().max(0.01) + 1e-4);
        let unshifted = g.aligned(half);
        let mut normal = unshifted;
        normal.origin -= super::exact_probe::sticky_shift(link.as_ref());
        let border = clip.border();
        let rect = |b: Bounds<Pixels>| {
            [
                f32::from(b.left()) - border[0],
                f32::from(b.top()) - border[1],
                f32::from(b.right()) + border[2],
                f32::from(b.bottom()) + border[3],
            ]
        };
        super::BoxRecord {
            id,
            rect: rect(normal),
            clip,
            snap: (snap.align != [0; 2]).then(|| snap::Area {
                rect: rect(unshifted),
                snap,
                vertical,
                reverse,
            }),
        }
    }))
}

/// Line boxes contribute to scrollable overflow, never text-decoration ink.
pub(crate) fn record_text(
    frame: &Frame,
    id: u64,
    mut b: Bounds<Pixels>,
    sticky: Option<&super::exact_probe::StickyShift>,
) {
    b.origin -= super::exact_probe::sticky_shift(sticky);
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

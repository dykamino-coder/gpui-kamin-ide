//! Перенос допуска семьи корня, который prepaint поставил не туда, откуда его допуск
//! построен (поздняя статическая позиция): исходные точки, срезы и декорации едут
//! вместе с корнем, назначения остаются.

use crate::layout::fragment::decoration::Decoration;
use crate::layout::fragment::paint_source::Nested;
use gpui::{PaintSegmentSlice, Pixels, Point, Window};
use std::rc::Rc;

/// Final placement of the active continued root minus its admission origin.
pub(super) struct Shift(pub(super) Point<Pixels>);

pub(super) fn current(window: &Window) -> Point<Pixels> {
    window
        .render_context::<Shift>()
        .map_or_else(Point::default, |shift| shift.0)
}

fn slice(mut slice: PaintSegmentSlice, by: Point<Pixels>) -> PaintSegmentSlice {
    slice.placement.source = slice.placement.source + by;
    slice.source_clip.origin = slice.source_clip.origin + by;
    slice
}

pub(super) fn slices(
    slices: Rc<[PaintSegmentSlice]>,
    by: Point<Pixels>,
) -> Rc<[PaintSegmentSlice]> {
    if by == Point::default() {
        return slices;
    }
    slices.iter().map(|s| slice(*s, by)).collect()
}

pub(super) fn boxes(boxes: Rc<[Decoration]>, by: Point<Pixels>) -> Rc<[Decoration]> {
    if by == Point::default() {
        return boxes;
    }
    boxes
        .iter()
        .map(|b| {
            let mut b = *b;
            b.bounds.origin = b.bounds.origin + by;
            b.slice = slice(b.slice, by);
            b
        })
        .collect()
}

pub(super) fn nested(nested: Rc<Nested>, by: Point<Pixels>) -> Rc<Nested> {
    if by == Point::default() {
        return nested;
    }
    let mut extent = nested.extent;
    extent.origin = extent.origin + by;
    Rc::new(Nested {
        extent,
        routes: nested
            .routes
            .iter()
            .map(|r| {
                let mut r = *r;
                r.source = r.source + by;
                r
            })
            .collect(),
        families: nested
            .families
            .iter()
            .map(|(id, s)| (*id, slices(s.clone(), by)))
            .collect(),
        boxes: nested
            .boxes
            .iter()
            .map(|(id, b)| (*id, boxes(b.clone(), by)))
            .collect(),
        bare: nested.bare.clone(),
    })
}

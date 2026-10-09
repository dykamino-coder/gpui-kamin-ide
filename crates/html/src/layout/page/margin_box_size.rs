//! Intrinsic page margin dimensions remain distinct from auto stretch sizing.

use crate::flow::*;
use crate::value::Len;
use gpui::AvailableSpace;

pub(super) fn keyword(length: Option<Len>) -> bool {
    matches!(
        length,
        Some(Len::MinContent | Len::MaxContent | Len::FitContent)
    )
}

// CSS Page 3 §fixed-sizing constrains auto dimensions to the page margin;
// CSS Sizing 3 §sizing-values instead measures intrinsic sizing keywords.
// Blink LayoutEdgeMarginNode lays out the cross axis after fixing the main
// axis (page_container_layout_algorithm.cc:802-815).
pub(super) fn intrinsic(
    b: &mut MarginBox,
    horizontal: bool,
    other: Option<f32>,
    available: f32,
    window: &mut Window,
    cx: &mut App,
) -> Option<f32> {
    let length = b.intrinsic[usize::from(!horizontal)];
    if !keyword(length) {
        return None;
    }
    let other = other.map_or(AvailableSpace::MaxContent, |v| {
        AvailableSpace::Definite(px(v))
    });
    let mut measure = |axis| {
        let space = if horizontal {
            size(axis, other)
        } else {
            size(other, axis)
        };
        let measured = b.probe.layout_as_root_unrounded(space, window, cx);
        f32::from(if horizontal {
            measured.width
        } else {
            measured.height
        })
    };
    let max = measure(AvailableSpace::MaxContent);
    Some(match length {
        Some(Len::MinContent) => measure(AvailableSpace::MinContent),
        Some(Len::FitContent) => max.min(available).max(measure(AvailableSpace::MinContent)),
        _ => max,
    })
}

//! Внутренние размеры марджин-бокса листа (`min-content`/`max-content`/`fit-content`)
//! мерой пробного элемента по оси коробки; они отличаются от авто-размера, который
//! css-page-3 §fixed-sizing растягивает по полю листа. Поперечная ось меряется после
//! главной, как Blink `LayoutEdgeMarginNode`.

use crate::layout::page::page_stack::MarginBox;
use crate::style::values::value::Len;
use gpui::{App, AvailableSpace, Window, px, size};

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
    let mut measure = |axis| measure(b, horizontal, other, axis, window, cx);
    let max = measure(AvailableSpace::MaxContent);
    Some(match length {
        Some(Len::MinContent) => measure(AvailableSpace::MinContent),
        Some(Len::FitContent) => max.min(available).max(measure(AvailableSpace::MinContent)),
        _ => max,
    })
}

// CSS Page 3 §margin-box-terms: intrinsic contributions follow the box's
// inline axis, even on a physically vertical page edge. Blink
// page_container_layout_algorithm.cc:536-588 makes the same axis distinction.
pub(super) fn measure(
    b: &MarginBox,
    horizontal: bool,
    other: Option<f32>,
    axis: AvailableSpace,
    window: &mut Window,
    cx: &mut App,
) -> f32 {
    let inline = if b.vertical {
        if horizontal {
            other.map(Len::Px)
        } else {
            Some(match axis {
                AvailableSpace::MinContent => Len::MinContent,
                AvailableSpace::MaxContent => Len::MaxContent,
                AvailableSpace::Definite(v) => Len::Px(v.into()),
            })
        }
    } else {
        None
    };
    let mut probe = (b.probe)(inline);
    let other = other.map_or(AvailableSpace::MaxContent, |v| {
        AvailableSpace::Definite(px(v))
    });
    let space = if horizontal {
        size(axis, other)
    } else {
        size(other, axis)
    };
    let measured = probe.layout_as_root_unrounded(space, window, cx);
    if horizontal {
        measured.width.into()
    } else {
        measured.height.into()
    }
}

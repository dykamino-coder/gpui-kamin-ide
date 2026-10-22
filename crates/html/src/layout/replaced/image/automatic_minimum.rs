//! Автоматический минимум замещаемого элемента flex по пропорции content box.
//! CSS Flexbox 1 §4.5: поперечные ограничения зажимают предложение размера содержимого,
//! а его накрывают заданный главный размер и определённый главный максимум.

use crate::style::computed::{Computed, Overflow};
use crate::style::values::value::Len;

pub(crate) fn apply(native: &mut gpui::Style, c: &Computed, w: f32, h: f32, ratio: Option<f32>) {
    if !c.flex_item {
        return;
    }
    let vertical = c.flex_main_vertical;
    let (preferred, minimum, maximum, cross, cross_min, cross_max, overflow) = if vertical {
        (
            c.height,
            c.min_height,
            c.max_height,
            c.width,
            c.min_width,
            c.max_width,
            c.overflow_y,
        )
    } else {
        (
            c.width,
            c.min_width,
            c.max_width,
            c.height,
            c.min_height,
            c.max_height,
            c.overflow_x,
        )
    };
    if !matches!(minimum, None | Some(Len::Auto))
        || matches!(overflow, Some(Overflow::Hidden | Overflow::Scroll))
        || (matches!(cross, None | Some(Len::Auto))
            && matches!(cross_min, None | Some(Len::Auto))
            && matches!(cross_max, None | Some(Len::Auto)))
        || [cross, cross_min, cross_max, preferred, maximum]
            .into_iter()
            .any(|value| !matches!(value, None | Some(Len::Auto | Len::Px(_))))
    {
        return;
    }
    let Some(ratio) = ratio.filter(|r| r.is_finite() && *r > 0.0) else {
        return;
    };
    let border = c.borders();
    let sum = |edges: [Option<Len>; 4]| {
        edges.into_iter().try_fold(0.0, |sum, edge| match edge {
            None => Some(sum),
            Some(Len::Px(value)) => Some(sum + value),
            _ => None,
        })
    };
    let Some(x) = sum([c.padding.left, c.padding.right, border.left, border.right]) else {
        return;
    };
    let Some(y) = sum([c.padding.top, c.padding.bottom, border.top, border.bottom]) else {
        return;
    };
    let (main_edges, cross_edges) = if vertical { (y, x) } else { (x, y) };
    let px = |length, edges| match length {
        Some(Len::Px(value)) => Some(
            (value
                - if c.border_box == Some(true) {
                    edges
                } else {
                    0.0
                })
            .max(0.0),
        ),
        _ => None,
    };
    let low = px(cross_min, cross_edges).unwrap_or(0.0);
    let high = px(cross_max, cross_edges).unwrap_or(f32::INFINITY).max(low);
    let intrinsic_cross = if vertical { w } else { h };
    let used_cross = px(cross, cross_edges)
        .unwrap_or(intrinsic_cross)
        .clamp(low, high);
    let mut content = if vertical {
        used_cross / ratio
    } else {
        used_cross * ratio
    };
    if let Some(specified) = px(preferred, main_edges) {
        content = content.min(specified);
    }
    if let Some(maximum) = px(maximum, main_edges) {
        content = content.min(maximum);
    }
    let minimum = gpui::px(content + if native.content_box { 0.0 } else { main_edges }).into();
    if vertical {
        native.min_size.height = minimum;
    } else {
        native.min_size.width = minimum;
    }
    // The auto cross size follows the final flexed main size. Exposing the
    // ratio here would synthesize a definite cross size from the preferred
    // main size before flex shrinking; the measured leaf transfers it later.
    if matches!(cross, None | Some(Len::Auto)) {
        native.aspect_ratio = None;
    }
}

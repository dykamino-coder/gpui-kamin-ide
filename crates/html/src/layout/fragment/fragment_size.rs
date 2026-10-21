//! Мера коробки для фрагментации (`shape_full`): к мере содержимого (`shape_contents`)
//! применяются `min-height`/`max-height`, рамки и отступы, а точки разреза и монолиты режутся
//! по использованной высоте. Использованный блочный размер и протяжённость переполнения —
//! РАЗНЫЕ входы фрагментации: `max-height` меняет размер коробки и при видимом переполнении,
//! переполнившие потомки остаются параллельным потоком, а не отодвигают следующего брата
//! (только в режиме протяжённости, `ShapeCx::unclamped`, мера растёт до естественной);
//! `min-height` побеждает, когда больше `max-height`.
//! Таблица меру высоты ведёт сама (CSS Tables 3).

use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::layout::fragment::oof_reach::Measure;
use crate::layout::fragment::shape_contents::shape_contents;
use crate::layout::fragment::table_bands::table_box;
use crate::layout::fragment::{Shape, ShapeCx};
use crate::style::computed::{Computed, Overflow};
use crate::style::values::value::Len;

pub(crate) fn shape_full(
    element: &Element,
    depth: u8,
    cx: ShapeCx,
    vcx: ValueCx<'_>,
) -> Option<Shape> {
    measure(element, depth, cx, vcx).map(|m| m.shape)
}

/// Мера фрагментации коробки (`shape_full`) вместе с её внепоточной
/// досягаемостью и собственным размером (`Measure`).
pub(crate) fn measure(
    element: &Element,
    depth: u8,
    cx: ShapeCx,
    vcx: ValueCx<'_>,
) -> Option<Measure> {
    let cx = ShapeCx {
        legacy_oof: cx.legacy_oof
            || element.style.bdb_clone
            || crate::layout::multicol::spanner::multicol_container(&element.style),
        ..cx
    };
    let mut m = shape_contents(element, depth, cx, vcx)?;
    // Table height is a minimum of the row grid (CSS Tables 3), and its
    // specialised measurement already owns that sizing/distribution pass.
    if table_box(element) {
        return Some(m);
    }
    let shape = &mut m.shape;
    let resolve = |value: Option<Len>, padding: bool| match value {
        Some(Len::Px(1.0)) if padding && cx.cell_pad.is_some() => cx.cell_pad.unwrap(),
        Some(Len::Px(v)) => v,
        Some(Len::Vw(k)) => cx.viewport.map_or(0.0, |v| k * v.0),
        Some(Len::Vh(k)) => cx.viewport.map_or(0.0, |v| k * v.1),
        _ => 0.0,
    };
    let border = element.style.borders();
    let decoration = resolve(element.style.padding.top, true)
        + resolve(element.style.padding.bottom, true)
        + resolve(border.top, false)
        + resolve(border.bottom, false);
    let height = constrain(
        shape.height,
        &element.style,
        decoration,
        cx.viewport,
        cx.unclamped,
    );
    if height < shape.height {
        shape
            .cuts
            .retain(|&(need, _)| need > 0.01 && need < height - 0.01);
        shape
            .forced
            .retain(|&forced| forced > 0.01 && forced < height - 0.01);
        shape.solid.retain(|&(start, _)| start < height - 0.01);
        for (_, end) in &mut shape.solid {
            *end = end.min(height);
        }
    }
    shape.height = height;
    Some(m)
}

pub(super) fn border_size(value: f32, style: &Computed, decoration: f32) -> f32 {
    if style.border_box == Some(true) {
        value.max(decoration)
    } else {
        value.max(0.0) + decoration
    }
}

pub(super) fn constrain(
    natural_border_size: f32,
    style: &Computed,
    decoration: f32,
    viewport: Option<(f32, f32)>,
    overflow_extent: bool,
) -> f32 {
    let resolve = |length: Option<Len>| match length {
        Some(Len::Px(v)) => Some(v),
        Some(Len::Vw(k)) => viewport.map(|v| k * v.0),
        Some(Len::Vh(k)) => viewport.map(|v| k * v.1),
        _ => None,
    };
    let border_size = |v: f32| border_size(v, style, decoration);
    let minimum = resolve(style.min_height).map(border_size);
    let maximum = resolve(style.max_height).map(border_size);
    let clips = matches!(style.overflow_y, Some(Overflow::Hidden | Overflow::Clip));
    if overflow_extent && !clips {
        return natural_border_size.max(minimum.unwrap_or(0.0));
    }
    // Explicit height has already been resolved by the content measurement.
    // Do not apply it again after that pass has accounted for positioned
    // descendants: their extent also contributes to column balancing.
    let mut used = natural_border_size;
    if let Some(maximum) = maximum {
        used = used.min(maximum);
    }
    used.max(minimum.unwrap_or(0.0)).max(decoration)
}

#[cfg(test)]
mod tests;

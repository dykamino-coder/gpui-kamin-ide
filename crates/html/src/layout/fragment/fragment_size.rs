//! TRANSITIONAL(F2-7): Estimator применяет ограничения к гипотетической форме; удалить в P22 после удаления всех production readers в P21, сохранив реальные geometry types.
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
use crate::style::computed::{Computed, LenCx, Overflow};
use crate::style::values::value::Len;

#[cfg_attr(feature = "wpt-harness", track_caller)]
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
#[cfg_attr(feature = "wpt-harness", track_caller)]
pub(crate) fn measure(
    element: &Element,
    depth: u8,
    cx: ShapeCx,
    vcx: ValueCx<'_>,
) -> Option<Measure> {
    #[cfg(feature = "wpt-harness")]
    if let Some(probe) = cx
        .probe
        .or_else(|| vcx.lines.map(|lines| &lines.opts.frag.probe))
    {
        probe.estimate(if cx.paged {
            "pages.estimate"
        } else {
            "columns.estimate"
        });
    }
    let cx = ShapeCx {
        legacy_oof: cx.legacy_oof
            || element.style.used().bdb_clone
            || crate::layout::multicol::spanner::multicol_container(&element.style.used()),
        ..cx
    };
    let mut m = shape_contents(element, depth, cx, vcx)?;
    // Table height is a minimum of the row grid (CSS Tables 3), and its
    // specialised measurement already owns that sizing/distribution pass.
    if table_box(element) {
        return Some(m);
    }
    let shape = &mut m.shape;
    // Рамка и отступ: нерешённое ничего не добавляет к мере.
    let lens = LenCx::box_viewport(cx.viewport, &element.style.used());
    let resolve = |value: Option<Len>| lens.px_lossy_zero(value, vcx);
    let border = element.style.used().borders();
    let decoration = resolve(element.style.used().padding.top)
        + resolve(element.style.used().padding.bottom)
        + resolve(border.top)
        + resolve(border.bottom);
    let height = constrain(
        shape.height,
        &element.style.used(),
        decoration,
        cx.viewport,
        cx.unclamped,
        vcx,
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
    vcx: ValueCx<'_>,
) -> f32 {
    // Предел, который не решается, не ограничивает.
    let lens = LenCx::box_viewport(viewport, style);
    let resolve = |length: Option<Len>| lens.px_strict(length, vcx);
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

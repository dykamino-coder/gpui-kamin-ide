//! Раскладка и prepaint одной копии куска плана и полос повтора таблицы — каждая в
//! своём кадре: копия может нести вложенный многоколоночник, и его prepaint идёт
//! внутри этих кадров.

use crate::layout::fragment::types::Frag;
use gpui::{App, Pixels, Window, point, px, size};

/// Полосы повтора таблицы — своими копиями (`Repeat::place_bands`).
#[inline(never)]
pub(super) fn prepaint_bands(
    r: &mut crate::layout::fragment::types::Repeat,
    f: &Frag,
    at: gpui::Point<Pixels>,
    (col_w, full_h): (f32, f32),
    window: &mut Window,
    cx: &mut App,
) {
    r.place_bands(f, |el, band| {
        el.layout_as_root_at(
            point(at.x, at.y + px(band)),
            size(
                gpui::AvailableSpace::Definite(px(col_w)),
                gpui::AvailableSpace::Definite(px(full_h)),
            ),
            window,
            cx,
        );
        el.prepaint_at(point(px(0.0), px(0.0)), window, cx);
    });
}

/// Копия раскладывается ЦЕЛИКОМ и поднимается на срез: видимой её часть делает
/// маска в отрисовке. Свой кадр: раскладка копии не держит место в цепочке
/// вложенных prepaint.
#[inline(never)]
pub(super) fn layout_frag(
    el: &mut gpui::AnyElement,
    at: gpui::Point<Pixels>,
    (col_w, full_h): (f32, f32),
    rtl_root_in_ltr: bool,
    window: &mut Window,
    cx: &mut App,
) -> gpui::Size<Pixels> {
    let avail = size(
        gpui::AvailableSpace::Definite(px(col_w)),
        gpui::AvailableSpace::Definite(px(full_h)),
    );
    let laid = el.layout_as_root_at(at, avail, window, cx);
    // Корень rtl taffy ставит у правого края колонки; содержащий блок ltr —
    // вернуть к левому (`Par::rtl_root_in_ltr`).
    if rtl_root_in_ltr && f32::from(laid.width) < col_w {
        let back = px(col_w - f32::from(laid.width));
        return el.layout_as_root_at(point(at.x - back, at.y), avail, window, cx);
    }
    laid
}

/// Prepaint копии на её месте — в цепочке вложенных многоколоночников, поэтому
/// без собственных крупных локальных.
#[inline(never)]
pub(super) fn prepaint_frag(
    el: &mut gpui::AnyElement,
    paint: &crate::layout::cx::deferred::Deferred,
    scopes: &crate::hooks::SlotScopes,
    mapping: Option<(
        usize,
        gpui::Point<Pixels>,
        crate::layout::fragment::static_coordinates::Places,
    )>,
    segment: gpui::PaintSegmentId,
    window: &mut Window,
    cx: &mut App,
) {
    crate::layout::fragment::static_coordinates::with(paint, mapping, || {
        crate::hooks::with_copy_hits(segment, scopes, window, |window| {
            el.prepaint_at(point(px(0.0), px(0.0)), window, cx)
        });
    });
}

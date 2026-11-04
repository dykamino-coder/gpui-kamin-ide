//! Раскладка и prepaint одной копии куска плана и полос повтора таблицы — каждая в
//! своём кадре: копия может нести вложенный многоколоночник, и его prepaint идёт
//! внутри этих кадров.

use crate::layout::fragment::types::Frag;
use gpui::{App, Pixels, Window, point, px, size};

/// Полосы повтора таблицы — своими копиями (`Repeat::place_bands`).
#[inline(never)]
pub(super) async fn prepaint_bands(
    r: &mut crate::layout::fragment::types::Repeat,
    f: &Frag,
    at: gpui::Point<Pixels>,
    (col_w, full_h): (f32, f32),
    window: &mut Window,
    cx: &mut App,
    stack: &mut gpui::PaintStack,
) {
    r.place_bands_async(f, async |el, band| {
        el.layout_as_root_at_async(
            point(at.x, at.y + px(band)),
            size(
                gpui::AvailableSpace::Definite(px(col_w)),
                gpui::AvailableSpace::Definite(px(full_h)),
            ),
            window,
            cx,
            stack,
        )
        .await;
        el.prepaint_at_async(point(px(0.0), px(0.0)), window, cx, stack)
            .await;
    })
    .await;
}

/// Копия раскладывается ЦЕЛИКОМ и поднимается на срез: видимой её часть делает
/// маска в отрисовке. Свой кадр: раскладка копии не держит место в цепочке
/// вложенных prepaint.
#[inline(never)]
pub(super) async fn layout_frag(
    el: &mut gpui::AnyElement,
    at: gpui::Point<Pixels>,
    (col_w, full_h): (f32, f32),
    rtl_root_in_ltr: bool,
    window: &mut Window,
    cx: &mut App,
    stack: &mut gpui::PaintStack,
) -> gpui::Size<Pixels> {
    let avail = size(
        gpui::AvailableSpace::Definite(px(col_w)),
        gpui::AvailableSpace::Definite(px(full_h)),
    );
    let laid = el
        .layout_as_root_at_async(at, avail, window, cx, stack)
        .await;
    // Корень rtl taffy ставит у правого края колонки; содержащий блок ltr —
    // вернуть к левому (`Par::rtl_root_in_ltr`).
    if rtl_root_in_ltr && f32::from(laid.width) < col_w {
        let back = px(col_w - f32::from(laid.width));
        return el
            .layout_as_root_at_async(point(at.x - back, at.y), avail, window, cx, stack)
            .await;
    }
    laid
}

/// Prepaint копии на её месте — в цепочке вложенных многоколоночников, поэтому
/// без собственных крупных локальных.
#[inline(never)]
pub(super) async fn prepaint_frag(
    el: &mut gpui::AnyElement,
    paint: &crate::layout::cx::deferred::Deferred,
    (scopes, segment): (&crate::hooks::SlotScopes, gpui::PaintSegmentId),
    mapping: Option<(
        usize,
        gpui::Point<Pixels>,
        crate::layout::fragment::static_coordinates::Places,
    )>,
    window: &mut Window,
    cx: &mut App,
    stack: &mut gpui::PaintStack,
) {
    crate::layout::fragment::static_coordinates::with_async(paint, mapping, async || {
        crate::hooks::with_copy_hits_async(segment, scopes, window, async |window| {
            el.prepaint_at_async(point(px(0.0), px(0.0)), window, cx, stack)
                .await
        })
        .await;
    })
    .await;
}

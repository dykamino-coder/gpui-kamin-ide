//! Позднее размещение (LatePlace): заместитель ждёт места в строке и сдвигается к нему при
//! prepaint.

use super::SpotCell;
use crate::hooks::Hooks;
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Window, px};
mod shift;
pub(super) use shift::late_shift;
mod spot;
pub use spot::spot_place;
pub use spot::spot_probe;

pub struct LatePlace {
    pub(crate) child: AnyElement,
    pub(crate) spot: SpotCell,
}

impl Hooks for LatePlace {
    type Layout = LayoutId;
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, Self::Layout) {
        let layout_id = self.child.request_layout(window, cx);
        (layout_id, layout_id)
    }

    fn prepaint(
        &mut self,
        _bounds: Bounds<Pixels>,
        layout_id: &mut Self::Layout,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::Prepaint {
        let bounds = Bounds {
            origin: window.layout_origin_unrounded(*layout_id),
            size: window.layout_size_unrounded(*layout_id),
        };
        let placed = placed_bounds(bounds, self.spot.get());
        let child = &mut self.child;
        window.set_layout_placed_origin(*layout_id, placed.origin);
        child.prepaint_at(gpui::point(px(0.0), px(0.0)), window, cx);
    }

    fn paint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _request: &mut Self::Layout,
        _prepaint: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        gpui::paint_reopen(|| self.child.paint(window, cx));
    }
}

/// Та же статическая позиция нужна layout-пробе до запуска prepaint.
pub(crate) fn placed_bounds(mut bounds: Bounds<Pixels>, now: super::Spot) -> Bounds<Pixels> {
    let shift = late_shift(bounds, now);
    // Ось, которую задал содержащий блок, раскладка уже разрешила — щуп
    // её не трогает; по свободной оси к дырке добавляется поле (§10.3.7:
    // от статической позиции коробку отодвигает `margin`).
    let shift = gpui::point(
        if now.fixed_axes.0 {
            px(0.0)
        } else {
            shift.x + px(now.free_margin.0)
        },
        if now.fixed_axes.1 {
            px(0.0)
        } else {
            shift.y + px(now.free_margin.1)
        },
    );
    bounds.origin += shift;
    bounds
}

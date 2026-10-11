//! Позднее размещение (LatePlace): заместитель ждёт места в строке и сдвигается к нему при prepaint.

use super::SpotCell;
use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window, px,
};
mod shift;
pub(super) use shift::late_shift;
mod spot;
pub use spot::spot_place;
pub use spot::spot_probe;

pub struct LatePlace {
    pub(crate) child: Option<AnyElement>,
    pub(crate) spot: SpotCell,
}

impl Element for LatePlace {
    type RequestLayoutState = LayoutId;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        let layout_id = self.child.as_mut().unwrap().request_layout(window, cx);
        (layout_id, layout_id)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        layout_id: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let bounds = Bounds {
            origin: window.layout_origin_unrounded(*layout_id),
            size: window.layout_size_unrounded(*layout_id),
        };
        let now = self.spot.get();
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
        let child = self.child.as_mut().unwrap();
        window.set_layout_placed_origin(*layout_id, bounds.origin + shift);
        child.prepaint_at(gpui::point(px(0.0), px(0.0)), window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request: &mut LayoutId,
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        // Заместитель своего контекста краски не заводит (сдвиг — в
        // подготовке): собиратель шага 8 (`gpui::PaintLast`) проходит его
        // насквозь, как обычную коробку.
        let child = self.child.as_mut().unwrap();
        gpui::paint_reopen(|| child.paint(window, cx));
    }
}

impl IntoElement for LatePlace {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

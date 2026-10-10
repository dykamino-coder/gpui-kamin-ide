//! Painting for element; split out to keep the owning module within 250 lines.

mod flat;

use crate::text::paragraph::*;
use gpui::{
    App, Bounds, Element, GlobalElementId, Hitbox, InspectorElementId, LayoutId, Pixels, Window,
    size,
};

impl Paragraph {
    pub(crate) fn paint_impl(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut LayoutId,
        hitbox: &mut Option<Hitbox>,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Поворачивается текст внутри уже рассчитанной по физическим осям коробки.
        if self.vertical {
            let bounds = self.vertical_paint_bounds(
                bounds,
                self.vertical_layout_origin,
                window.scale_factor(),
            );
            let matrix = self.vertical_transform(bounds, window.scale_factor());
            // Flat inline/block axes match the painted physical extent.
            let flat = Bounds {
                origin: bounds.origin,
                size: size(bounds.size.height, bounds.size.width),
            };
            let mut inner = std::mem::replace(self, Paragraph::empty());
            inner.vertical = false;
            let selection = inner
                .selection_vertical
                .replace((bounds, inner.vertical_ccw));
            window.with_transformation(matrix, |window| {
                inner.paint(id, _inspector_id, flat, _state, hitbox, window, cx);
            });
            inner.selection_vertical = selection;
            inner.vertical = true;
            *self = inner;
            return;
        }
        self.paint_flat(id, bounds, hitbox, window, cx);
    }
}

//! Проба border box элемента и его хвостовых полей для линеек промежутков.

use crate::layout::multicol::gap_rules::GapItems;
use gpui::{AnyElement, Bounds, px};

/// CSS Gaps 1 §gap-grid/§gap-flex: промежуток идёт между рамочными краями.
/// Середина промежутка считается до округления к точкам устройства.
pub fn gap_item_probe(items: GapItems, border: [f32; 4], margins: [f32; 2]) -> AnyElement {
    items.borrow().items.record_with(move |g, _| {
        let b = g.unrounded;
        let [t, r, bottom, l] = border;
        (
            Bounds {
                origin: gpui::point(b.origin.x - px(l), b.origin.y - px(t)),
                size: gpui::size(b.size.width + px(l + r), b.size.height + px(t + bottom)),
            },
            margins,
        )
    })
}

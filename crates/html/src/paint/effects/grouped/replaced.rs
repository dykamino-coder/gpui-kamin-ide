//! Группа замещаемого элемента с фигурными углами (`corner-shape`):
//! содержимое режется внутренним контуром, поэтому кольцо рамки ложится
//! поверх картинки в content-box (css-borders-4 §corner-shape-rendering).

use crate::style::computed::Computed;
use crate::style::values::value::px_lossy_zero;
use gpui::AnyElement;

/// Замещаемый элемент с фигурными углами: его содержимое режется внутренним
/// контуром — кольцо рамки рисуется поверх него в content-box.
pub(crate) fn grouped_replaced(el: AnyElement, c: &Computed) -> AnyElement {
    let side = px_lossy_zero;
    let ring =
        crate::paint::decorations::shaped_ring::spec(c).map(|(spec, colour, [t, r, b, l])| {
            let p = c.padding;
            let insets = [
                t + side(p.top),
                r + side(p.right),
                b + side(p.bottom),
                l + side(p.left),
            ];
            (spec, colour, insets)
        });
    super::grouped_with(el, c, ring)
}

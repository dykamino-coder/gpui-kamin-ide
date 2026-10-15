//! Native controls share the replaced-content natural measurement mechanism.

use gpui::{AnyElement, Div, px, size};

pub(super) fn control(paint: Div, natural: [f32; 2]) -> AnyElement {
    crate::layout::replaced::natural_control(paint, size(px(natural[0]), px(natural[1])))
}

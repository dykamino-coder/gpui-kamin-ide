//! Used content box из адаптера P01 для единиц контейнера (css-conditional-5 §6.1).
use crate::dom::Element;
use crate::layout::fragment::geometry::Node;
use crate::style::cascade::materialize::ComputedSlot;
use crate::style::computed::BoxStyle;
use gpui::{Div, LayoutId, LayoutObserver, Window};

#[cfg(test)]
mod box_tests;
#[cfg(test)]
mod fixture;
#[cfg(test)]
mod tests;

pub(super) fn observe(div: Div, element: &Element, style: &BoxStyle) -> Div {
    let axes = style.query_container_axes();
    if axes == (false, false) || element.marks.anonymous_wrapper {
        return div;
    }
    div.observe_layout(ContainerInput {
        slot: element.style.slot().clone(),
        axes,
        zoom: style.zoom_eff.unwrap_or(1.0),
        node: None,
    })
}

struct ContainerInput {
    slot: ComputedSlot,
    axes: (bool, bool),
    zoom: f32,
    node: Option<Node>,
}

impl LayoutObserver for ContainerInput {
    fn request_layout(&mut self, id: LayoutId, window: &Window) {
        self.node = Some(Node::capture(id, window));
    }

    fn prepaint(&mut self, window: &mut Window) -> bool {
        let Some(layout) = self.node.and_then(|node| node.read(window)) else {
            // Intrinsic/изолированный замер не публикует базу единиц контейнера.
            return true;
        };
        let size = layout.content_size;
        let input = (
            self.axes.0.then_some(f32::from(size.width) / self.zoom),
            self.axes.1.then_some(f32::from(size.height) / self.zoom),
        );
        if self.slot.set_container_size(input) {
            window.request_layout_retry();
            // Геометрия потомков посчитана с прежним входом предка.
            return false;
        }
        true
    }
}

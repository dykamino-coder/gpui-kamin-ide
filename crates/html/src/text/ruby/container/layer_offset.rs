//! Move an outer annotation beyond descendant ruby containers, CSS Ruby 1 §3.2.1.

use crate::hooks::{IntoHooked, Wrap};
use crate::text::ruby::layers::Layers;
use gpui::{AnyElement, App, Bounds, Pixels, Window, point, px};
use std::rc::Rc;

pub(super) fn offset(child: AnyElement, layers: &[Rc<Layers>], under: bool) -> AnyElement {
    Offset {
        child,
        layers: layers
            .iter()
            .map(|layer| (layer.clone(), layer.last_index()))
            .collect(),
        under,
    }
    .hooked()
}

struct Offset {
    child: AnyElement,
    layers: Vec<crate::text::ruby::layers::LayerAttachment>,
    under: bool,
}

impl Wrap for Offset {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn prepaint(&mut self, _bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let h = self
            .layers
            .iter()
            .map(|(layer, before)| layer.before(self.under, *before, window))
            .fold(0.0f32, f32::max);
        let shift = point(px(0.0), px(if self.under { h } else { -h }));
        window.with_exact_element_offset(shift, |window| self.child.prepaint(window, cx));
    }
}

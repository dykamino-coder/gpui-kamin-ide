//! Background layers painted by their own box, not built as child elements.
//!
//! CSS 2.1 Appendix E paints a box's background (step 2 of its stacking context, or the
//! block's own background step) as part of the box itself; backgrounds are not boxes and take
//! no part in layout. Blink's `BoxPainter` and Servo's display-list builder paint them from
//! the box's fragment. A background layer that is an absolutely positioned child element costs
//! an element, a layout node and a prepaint/paint traversal step per box; here the box's own
//! painter paints it at exactly the geometry that child would have had
//! (`Window::inset_layer_bounds`: the same absolute edge rounding).

use crate::layout::fragment::geometry::Node;
use crate::paint::background::exact_layer::{InsetLayer, PaintGeometry};
use gpui::{AnyElement, BoxPainter, Div, LayoutId, LayoutObserver, Window};
use std::cell::Cell;
use std::rc::Rc;

/// Geometry of each layer in the painted tree, written in prepaint, read by the box's paint.
type Geometry = Rc<Cell<Vec<Option<PaintGeometry>>>>;

struct Observer {
    node: Option<Node>,
    insets: Vec<[f32; 4]>,
    geometry: Geometry,
}

impl LayoutObserver for Observer {
    fn request_layout(&mut self, id: LayoutId, window: &Window) {
        self.node = Some(Node::capture(id, window));
    }

    fn prepaint(&mut self, window: &mut Window) -> bool {
        let mut geometry = self.geometry.take();
        geometry.clear();
        // A layout id from another (measuring) tree is not this box in the painted tree.
        if let Some(node) = self.node.and_then(|node| node.id_in(window)) {
            // The edges a layer child would read as its parent's are this box's own.
            let edges = window.box_edges(node);
            geometry.extend(self.insets.iter().map(|insets| {
                window
                    .inset_layer_bounds(node, *insets)
                    .map(|(snapped, exact)| PaintGeometry::of_layer(snapped, exact, edges))
            }));
        }
        self.geometry.set(geometry);
        true
    }
}
/// A layer goes to the box's own paint only while no decoration element precedes it, so the
/// painting order of all decorations stays the order of the decoration list.
pub(super) fn push(
    out: &mut Vec<AnyElement>,
    own: Option<&mut Vec<InsetLayer>>,
    layer: InsetLayer,
) {
    match own {
        Some(own) if out.is_empty() => own.push(layer),
        _ => out.push(layer.into_element()),
    }
}

/// Whether the box may paint its background layers itself: its children are not clipped or
/// scrolled by it (an absolutely positioned layer child would be), and no other layout
/// observer is attached to it.
pub(crate) fn eligible(div: &mut Div) -> bool {
    use gpui::Styled as _;
    let visible = |o: Option<gpui::Overflow>| matches!(o, None | Some(gpui::Overflow::Visible));
    let style = div.style();
    visible(style.overflow.x) && visible(style.overflow.y)
}

/// Paint `layers` (bottom first) after the box's own background and border quad, before its
/// children — the order in which they were painted as its first children.
pub(crate) fn attach(div: Div, layers: Vec<InsetLayer>) -> Div {
    if layers.is_empty() {
        return div;
    }
    let geometry = Geometry::default();
    let observer = Observer {
        node: None,
        insets: layers.iter().map(InsetLayer::insets).collect(),
        geometry: geometry.clone(),
    };
    let layers = Cell::new(layers);
    div.observe_layout(observer)
        .paint_box_with(BoxPainter(Rc::new(move |style, bounds, window, cx| {
            style.paint_box(bounds, window, cx);
            let (mut painted, places) = (layers.take(), geometry.take());
            for (layer, place) in painted.iter_mut().zip(&places) {
                if let Some(place) = place {
                    layer.paint(*place, window);
                }
            }
            layers.set(painted);
            geometry.set(places);
            true
        })))
}
#[cfg(test)]
mod tests {
    use super::*;
    use gpui::IntoElement as _;

    #[test]
    fn only_layers_before_every_decoration_element_go_to_the_box() {
        let layer = || InsetLayer::new([0.0; 4], |_, _| {});
        let (mut out, mut own) = (Vec::new(), Vec::new());
        push(&mut out, Some(&mut own), layer());
        push(&mut out, Some(&mut own), layer());
        assert_eq!((out.len(), own.len()), (0, 2));
        out.push(gpui::div().into_any_element());
        push(&mut out, Some(&mut own), layer());
        assert_eq!(
            (out.len(), own.len()),
            (2, 2),
            "a later layer stays an element"
        );
        push(&mut out, None, layer());
        assert_eq!(
            out.len(),
            3,
            "a box that cannot paint layers keeps elements"
        );
    }
}

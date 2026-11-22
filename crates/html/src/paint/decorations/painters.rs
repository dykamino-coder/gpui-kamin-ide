//! The decorations of a box painted without layout: what a box that is the
//! whole content of its composition group paints again with its own paint
//! (`gpui::Window::rerecord_layer_group`, `document::retained::box_repaint`).
//!
//! The same emission as `decorations` (`emit`), so both list the same layers
//! in the same order; a layer that paints from its box alone (`ExactLayer`:
//! background tiles, the single-corner fill) is kept as a painter, and any
//! other element makes the list unpaintable, so the frame is built instead.

use super::{Out, emit};
use crate::layout::cx::FrameCx;
use crate::paint::background::exact_layer::ExactLayer;
use crate::style::computed::BoxStyle;
use gpui::{AnyElement, Bounds, Pixels, Window};

/// The decorations of a box as painters, in paint order.
pub(crate) struct Painters(Vec<ExactLayer>);

/// What `emit` gives: painters, and elements that only layout can place.
#[derive(Default)]
struct Sink {
    layers: Vec<ExactLayer>,
    elements: Vec<AnyElement>,
}

impl Out for Sink {
    fn exact(&mut self, layer: ExactLayer) {
        self.layers.push(layer);
    }

    fn elements(&mut self) -> &mut Vec<AnyElement> {
        &mut self.elements
    }
}

/// The decorations `decorations` gives the box, as painters; `None` when one
/// of them is an element that needs layout, or the box's corners clip it
/// (`effects::corner_clip`, which paints the box otherwise).
pub(crate) fn decoration_painters(
    c: &BoxStyle,
    empty: bool,
    frame: FrameCx<'_>,
    cx: Option<&crate::layout::cx::LayoutCx>,
) -> Option<Painters> {
    if crate::paint::effects::corner_clip::insets(c, frame.values).is_some() {
        return None;
    }
    let mut sink = Sink::default();
    emit(c, empty, frame, cx, &mut sink);
    (sink.elements.is_empty() && sink.layers.iter().all(|layer| !layer.needs_layout()))
        .then_some(Painters(sink.layers))
}

impl Painters {
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Paint every layer from the parent's padding box `padding`. `None`,
    /// with nothing painted, when layout would round a layer's box to another
    /// one: its edges are off the device grid at `scale`.
    pub(crate) fn paint(
        &mut self,
        padding: Bounds<Pixels>,
        scale: f32,
        window: &mut Window,
    ) -> Option<()> {
        let boxes: Vec<Bounds<Pixels>> = self.0.iter().map(|l| l.bounds_in(padding)).collect();
        if !boxes.iter().all(|b| on_grid(*b, scale)) {
            return None;
        }
        for (layer, bounds) in self.0.iter_mut().zip(boxes) {
            layer.paint_at(bounds, window);
        }
        Some(())
    }
}

/// Whether every edge of `b` lies on the device-pixel grid at `scale` and
/// its size is not negative: layout then rounds it to itself.
fn on_grid(b: Bounds<Pixels>, scale: f32) -> bool {
    let on = |v: Pixels| {
        let d = f32::from(v) * scale;
        (d - d.round()).abs() <= 1e-3
    };
    b.size.width >= Pixels::ZERO
        && b.size.height >= Pixels::ZERO
        && [b.left(), b.top(), b.right(), b.bottom()]
            .into_iter()
            .all(on)
}

#[cfg(test)]
mod tests {
    use super::on_grid;
    use gpui::{Bounds, point, px, size};

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<gpui::Pixels> {
        Bounds {
            origin: point(px(x), px(y)),
            size: size(px(w), px(h)),
        }
    }

    #[test]
    fn a_box_off_the_device_grid_is_not_painted_without_layout() {
        assert!(on_grid(rect(31., 47., 180., 140.), 1.0));
        assert!(on_grid(rect(10.5, 4., 20., 3.5), 2.0));
        assert!(!on_grid(rect(10.5, 4., 20., 3.), 1.0));
        assert!(!on_grid(rect(0., 0., 20., 3.25), 2.0));
        // Layout gives a negative size no box.
        assert!(!on_grid(rect(0., 0., -2., 3.), 1.0));
    }
}

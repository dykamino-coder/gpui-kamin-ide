//! Единый прозрачный элемент пробы: полный padding box и две системы координат.

use super::Sink;
use crate::hooks::{Hooks, IntoHooked};
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Window, px};

pub(crate) fn full_box() -> gpui::Style {
    let mut style = gpui::Style {
        position: gpui::Position::Absolute,
        ..Default::default()
    };
    style.inset.top = px(0.).into();
    style.inset.left = px(0.).into();
    style.size = gpui::size(gpui::relative(1.).into(), gpui::relative(1.).into());
    style
}

#[derive(Clone, Copy)]
pub(crate) struct Geometry {
    pub(crate) id: LayoutId,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) unrounded: Bounds<Pixels>,
}
impl Geometry {
    /// Перемещённый вручную фрагмент использует prepaint-координаты, а не исходный layout.
    pub(crate) fn aligned(&self, tolerance: Pixels) -> Bounds<Pixels> {
        let near = |a: Pixels, b: Pixels| (a - b).abs() <= tolerance;
        if near(self.unrounded.left(), self.bounds.left())
            && near(self.unrounded.top(), self.bounds.top())
            && near(self.unrounded.right(), self.bounds.right())
            && near(self.unrounded.bottom(), self.bounds.bottom())
        {
            self.unrounded
        } else {
            self.bounds
        }
    }
    /// Кромки таблицы округляют ширину полосы один раз, независимо от округления ячейки.
    pub(crate) fn origin_aligned(&self, tolerance: Pixels) -> Bounds<Pixels> {
        if (self.unrounded.origin.x - self.bounds.origin.x).abs() <= tolerance
            && (self.unrounded.origin.y - self.bounds.origin.y).abs() <= tolerance
        {
            self.unrounded
        } else {
            self.bounds
        }
    }
}

type Read<T> = Box<dyn FnOnce(Geometry, &mut Window) -> T>;
struct Record<T> {
    sink: Sink<T>,
    read: Option<Read<T>>,
    style: gpui::Style,
}
pub(super) fn prepaint<T: 'static>(
    sink: Sink<T>,
    style: gpui::Style,
    read: impl FnOnce(Geometry, &mut Window) -> T + 'static,
) -> AnyElement {
    Record {
        sink,
        read: Some(Box::new(read)),
        style,
    }
    .hooked()
}
impl<T: 'static> Hooks for Record<T> {
    type Layout = LayoutId;
    type Prepaint = ();
    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, LayoutId) {
        let id = window.request_layout(self.style.clone(), [], cx);
        (id, id)
    }
    fn prepaint(
        &mut self,
        bounds: Bounds<Pixels>,
        id: &mut LayoutId,
        window: &mut Window,
        _: &mut App,
    ) {
        if let Some(read) = self.read.take() {
            let unrounded = Bounds {
                origin: window.layout_origin_unrounded(*id),
                size: window.layout_size_unrounded(*id),
            };
            self.sink.push(read(
                Geometry {
                    id: *id,
                    bounds,
                    unrounded,
                },
                window,
            ));
        }
    }
    fn paint(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut LayoutId,
        _: &mut (),
        _: &mut Window,
        _: &mut App,
    ) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn relocated_fragments_keep_prepaint_coordinates_and_fractional_extents() {
        let id = LayoutId::from(taffy::NodeId::from(1_u64));
        let raw = Bounds::new(
            gpui::point(px(10.2), px(20.3)),
            gpui::size(px(30.4), px(40.1)),
        );
        let snapped = Bounds::new(gpui::point(px(10.), px(20.)), gpui::size(px(31.), px(40.)));
        let mut g = Geometry {
            id,
            bounds: snapped,
            unrounded: raw,
        };
        assert_eq!(g.aligned(px(0.51)), raw);
        g.bounds.origin.x += px(100.);
        assert_eq!(g.aligned(px(0.51)), g.bounds);
        assert_eq!(g.origin_aligned(px(0.51)), g.bounds);
        g.bounds = snapped;
        g.bounds.size.width = px(50.);
        assert_eq!(g.aligned(px(0.51)), g.bounds);
        assert_eq!(g.origin_aligned(px(0.51)), raw);
    }
}

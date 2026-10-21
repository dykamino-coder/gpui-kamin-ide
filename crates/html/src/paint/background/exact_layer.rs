//! Слой фона, позиционируемый от неокруглённой коробки раскладки: элемент `ExactLayer`
//! запоминает привязанные и точные границы, а `positioning` отдаёт точные для расчёта фазы
//! плиток. CSS 2.1 §14.2 и css-backgrounds-3 §3.6 кладут фон в область позиционирования
//! коробки; Blink решает `background-position` от непривязанной области
//! (`background_image_geometry.cc`: «Unsnapped positioning area is used to derive quantities
//! ... such as phase and position») и привязывает только назначение. Канвас слоя получал
//! привязанную к пикселям коробку, и центрированная плитка уезжала до половины пикселя
//! устройства относительно той же плитки в коробке равной точной геометрии
//! (background-root-013a/b).

use crate::hooks::Hooks;
use gpui::{AnyElement, App, Bounds, IntoElement, LayoutId, Pixels, Styled as _, Window, px};

/// Snapped clip and unrounded positioning area of one paint callback.
#[derive(Clone, Copy)]
pub(crate) struct PaintGeometry {
    pub(crate) snapped: Bounds<Pixels>,
    exact: Option<Bounds<Pixels>>,
}

impl PaintGeometry {
    /// Resolve only this layer's box; a different area keeps its own geometry.
    pub(crate) fn positioning(self, bounds: Bounds<Pixels>) -> Bounds<Pixels> {
        match self.exact {
            Some(exact) if self.snapped == bounds => exact,
            _ => bounds,
        }
    }
}

type PaintFn = Box<dyn FnMut(PaintGeometry, &mut Window)>;

pub(crate) struct ExactLayer {
    child: AnyElement,
    paint: PaintFn,
    exact: Option<Bounds<Pixels>>,
}

impl ExactLayer {
    /// A layer laid out with the given `[top, right, bottom, left]` insets
    /// from the parent's padding box (negative = outward), painting with the
    /// unrounded box available through [`PaintGeometry::positioning`].
    pub(crate) fn inset(
        [t, r, b, l]: [f32; 4],
        paint: impl FnMut(PaintGeometry, &mut Window) + 'static,
    ) -> Self {
        Self {
            child: gpui::div()
                .absolute()
                .top(px(t))
                .right(px(r))
                .bottom(px(b))
                .left(px(l))
                .into_any_element(),
            paint: Box::new(paint),
            exact: None,
        }
    }
}

impl Hooks for ExactLayer {
    type Layout = LayoutId;
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, Self::Layout) {
        let layout_id = self.child.request_layout(window, cx);
        (layout_id, layout_id)
    }

    fn prepaint(
        &mut self,
        bounds: Bounds<Pixels>,
        layout_id: &mut Self::Layout,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::Prepaint {
        let exact = Bounds {
            origin: window.layout_origin_unrounded(*layout_id),
            size: window.layout_size_unrounded(*layout_id),
        };
        // Only an unrounded box within a device pixel of the snapped one is
        // the same box (another frame of reference otherwise).
        let near = |a: Pixels, b: Pixels| (a - b).abs() <= px(1.0);
        self.exact = (near(exact.left(), bounds.left())
            && near(exact.top(), bounds.top())
            && near(exact.right(), bounds.right())
            && near(exact.bottom(), bounds.bottom()))
        .then_some(exact);
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        _prepaint: &mut Self::Prepaint,
        window: &mut Window,
        _cx: &mut App,
    ) {
        (self.paint)(
            PaintGeometry {
                snapped: bounds,
                exact: self.exact,
            },
            window,
        );
    }
}

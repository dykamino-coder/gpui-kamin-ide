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
use gpui::{
    AnyElement, App, Bounds, Edges, IntoElement, LayoutId, Pixels, Styled as _, Window, px,
};

/// Snapped clip and unrounded positioning area of one paint callback.
#[derive(Clone, Copy)]
pub(crate) struct PaintGeometry {
    pub(crate) snapped: Bounds<Pixels>,
    exact: Option<Bounds<Pixels>>,
    pub(crate) parent_edges: Option<(Edges<Pixels>, Edges<Pixels>)>,
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

impl PaintGeometry {
    /// Geometry of a layer box: its snapped bounds, its unrounded bounds when within a
    /// device pixel of them (another frame of reference otherwise), and the used border and
    /// padding of the box the layer belongs to.
    pub(crate) fn of_layer(
        snapped: Bounds<Pixels>,
        exact: Bounds<Pixels>,
        parent_edges: Option<(Edges<Pixels>, Edges<Pixels>)>,
    ) -> Self {
        let near = |a: Pixels, b: Pixels| (a - b).abs() <= px(1.0);
        let exact = (near(exact.left(), snapped.left())
            && near(exact.top(), snapped.top())
            && near(exact.right(), snapped.right())
            && near(exact.bottom(), snapped.bottom()))
        .then_some(exact);
        Self {
            snapped,
            exact,
            parent_edges,
        }
    }
}

type PaintFn = Box<dyn FnMut(PaintGeometry, &mut Window)>;

/// A background layer laid out with [top, right, bottom, left] insets from its box's
/// padding box: painted by the box itself (decorations::box_layers) or, where the box
/// cannot, as an [ExactLayer] child element.
pub(crate) struct InsetLayer {
    insets: [f32; 4],
    paint: PaintFn,
}

impl InsetLayer {
    pub(crate) fn new(
        insets: [f32; 4],
        paint: impl FnMut(PaintGeometry, &mut Window) + 'static,
    ) -> Self {
        Self {
            insets,
            paint: Box::new(paint),
        }
    }

    pub(crate) fn insets(&self) -> [f32; 4] {
        self.insets
    }

    pub(crate) fn paint(&mut self, geometry: PaintGeometry, window: &mut Window) {
        (self.paint)(geometry, window)
    }

    pub(crate) fn into_element(self) -> AnyElement {
        use crate::hooks::IntoHooked as _;
        ExactLayer::boxed(self.insets, self.paint).hooked()
    }
}

pub(crate) struct ExactLayer {
    child: AnyElement,
    paint: PaintFn,
    exact: Option<Bounds<Pixels>>,
    parent_edges: Option<(Edges<Pixels>, Edges<Pixels>)>,
}

impl ExactLayer {
    /// A layer laid out with the given `[top, right, bottom, left]` insets
    /// from the parent's padding box (negative = outward), painting with the
    /// unrounded box available through [`PaintGeometry::positioning`].
    pub(crate) fn inset(
        [t, r, b, l]: [f32; 4],
        paint: impl FnMut(PaintGeometry, &mut Window) + 'static,
    ) -> Self {
        Self::boxed([t, r, b, l], Box::new(paint))
    }

    fn boxed([t, r, b, l]: [f32; 4], paint: PaintFn) -> Self {
        Self {
            child: gpui::div()
                .absolute()
                .top(px(t))
                .right(px(r))
                .bottom(px(b))
                .left(px(l))
                .into_any_element(),
            paint,
            exact: None,
            parent_edges: None,
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
        self.parent_edges = window.parent_box_edges(*layout_id);
        let exact = Bounds {
            origin: window.layout_origin_unrounded(*layout_id),
            size: window.layout_size_unrounded(*layout_id),
        };
        self.exact = PaintGeometry::of_layer(bounds, exact, None).exact;
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
                parent_edges: self.parent_edges,
            },
            window,
        );
    }
}

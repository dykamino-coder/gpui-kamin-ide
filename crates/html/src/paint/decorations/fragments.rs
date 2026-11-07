//! Собственные native-декорации из явной геометрии, без повторного prepaint дерева.

use crate::hooks::{Hooks, IntoHooked};
use crate::layout::cx::LayoutCx;
use crate::layout::fragment::decoration::Decoration;
use crate::style::computed::BoxStyle;
use gpui::{
    AnyElement, App, Bounds, LayoutId, ParentElement, Pixels, Style, Styled, Window, div, px,
};

/// Обычные декорации остаются теми же элементами общего painter; только геометрия
/// их контейнера приходит от фрагмента. Opacity и filter принадлежат исходному узлу.
pub(crate) struct Fragments {
    node: u64,
    style: BoxStyle,
    empty: bool,
    cx: LayoutCx,
}

pub(crate) fn layer(c: &BoxStyle, empty: bool, cx: &LayoutCx) -> AnyElement {
    Fragments {
        node: c.self_node,
        style: c.clone(),
        empty,
        cx: cx.clone(),
    }
    .hooked()
}

impl Hooks for Fragments {
    fn paint_boundary(&self) -> bool {
        false
    }
    type Layout = ();
    type Prepaint = Vec<(Decoration, AnyElement)>;

    async fn request_layout_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        _stack: &mut gpui::PaintStack,
    ) -> (LayoutId, ()) {
        let mut style: Style = Style::default();
        style.position = gpui::Position::Absolute;
        (window.request_layout(style, [], cx), ())
    }

    async fn prepaint_async(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> Self::Prepaint {
        let boxes = crate::paint::coordinator::segments::decorations(self.node, window);
        let mut fragments = Vec::with_capacity(boxes.len());
        for geometry in boxes.iter() {
            let frame = self.cx.frame();
            let own = &self.style;
            // Контейнер несёт border-box, поэтому абсолютные слои общего painter
            // отсчитываются от того же padding-box, что и у обычной коробки.
            let border = own.borders();
            let length = |v| crate::style::values::value::px_lossy_zero(v);
            let child = div()
                .relative()
                .w(geometry.bounds.size.width)
                .h(geometry.bounds.size.height)
                .border_t(px(length(border.top)))
                .border_r(px(length(border.right)))
                .border_b(px(length(border.bottom)))
                .border_l(px(length(border.left)))
                .children(super::decorations(own, self.empty, frame, Some(&self.cx)));
            let mut child = gpui::IntoElement::into_any_element(child);
            child
                .layout_as_root_async(
                    geometry.bounds.size.map(gpui::AvailableSpace::Definite),
                    window,
                    cx,
                    stack,
                )
                .await;
            child
                .prepaint_at_async(geometry.bounds.origin, window, cx, stack)
                .await;
            fragments.push((*geometry, child));
        }
        fragments
    }

    fn paint(
        &mut self,
        bounds: Bounds<Pixels>,
        layout: &mut (),
        fragments: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        crate::hooks::paint_on_stack(self, bounds, layout, fragments, window, cx);
    }

    fn paint_async<'a>(
        &'a mut self,
        _: Bounds<Pixels>,
        _: &'a mut (),
        fragments: &'a mut Self::Prepaint,
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut gpui::PaintStack,
    ) -> impl std::future::Future<Output = ()> + 'a {
        async move {
            let admitted = crate::paint::coordinator::segments::decorations(self.node, window);
            for (geometry, child) in fragments {
                let own = admitted.iter().any(|g| {
                    g.slice.placement.id == geometry.slice.placement.id
                        && g.slice.placement.ordinal == geometry.slice.placement.ordinal
                });
                if own {
                    crate::paint::coordinator::segments::decoration_async(
                        *geometry,
                        window,
                        async |window| child.paint_async(window, cx, stack).await,
                    )
                    .await;
                }
            }
        }
    }
}

//! SVG replacement: прозрачный наблюдатель того же layout id, без новой коробки.

use super::*;
use crate::hooks::{Hooks, IntoHooked};
use gpui::{AnyElement, App};

pub(super) fn build(e: &Element, opts: &crate::RenderOpts) -> Option<AnyElement> {
    Some(
        Svg {
            inner: crate::svg::element_with_html(e, opts)?,
            element: e.clone(),
            values: opts.cx.vcx().keep(),
        }
        .hooked(),
    )
}

struct Svg {
    inner: AnyElement,
    element: Element,
    values: crate::values::ValueDoc,
}

impl Hooks for Svg {
    type Layout = LayoutId;
    type Prepaint = ();

    fn paint_boundary(&self) -> bool {
        false
    }

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, LayoutId) {
        let id = self.inner.request_layout(window, cx);
        (id, id)
    }

    fn prepaint(
        &mut self,
        _: Bounds<Pixels>,
        id: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let b = Bounds::new(
            window.layout_origin_unrounded(*id),
            window.layout_size_unrounded(*id),
        );
        let style = self.element.style.used();
        if !crate::layout::replaced::replaced_content::svg_has_box(&style) {
            Collector::record(
                window,
                self.element.node_id,
                [b; 3],
                crate::layout::geometry::font(&style, 16., self.values.vcx()),
            );
        }
        if let Some(fonts) = Collector::svg_fonts(window) {
            for source in crate::resources::svg_document::geometry::query(
                &self.element,
                b.size.width.into(),
                b.size.height.into(),
                self.values.vcx(),
                fonts,
            ) {
                Collector::record_quad(
                    window,
                    source.node,
                    source
                        .corners
                        .map(|(x, y)| b.origin + gpui::point(gpui::px(x), gpui::px(y))),
                );
            }
        }
        self.inner.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut LayoutId,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.inner.paint(window, cx);
    }
}

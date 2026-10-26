//! Native-геометрия предка спаннера: ширина content box и раздельные кромки.
//! Щупы не публикуют LayoutId за пределами вложенного layout tree.

use crate::dom::Element;
use crate::hooks::{IntoHooked, Wrap};
use crate::layout::multicol::spanner::SpannerMetrics;
use crate::layout::options::RenderOpts;
use crate::style::cascade::materialize::box_style;
use crate::style::computed::BoxStyle;
use crate::style::values::value::Len;
use gpui::{
    AnyElement, App, AvailableSpace, IntoElement, LayoutId, ParentElement, Styled, Window, div, px,
    size,
};
use std::{cell::Cell, rc::Rc};

struct Capture {
    child: AnyElement,
    id: Rc<Cell<Option<LayoutId>>>,
}

impl Wrap for Capture {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }
    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> LayoutId {
        let id = self.child.request_layout(window, cx);
        self.id.set(Some(id));
        id
    }
}

pub(super) fn measure(
    child: &Element,
    parent: &BoxStyle,
    width: f32,
    opts: &RenderOpts,
    window: &mut Window,
    cx: &mut App,
) -> SpannerMetrics {
    window.with_nested_layout(|window| {
        let id = Rc::new(Cell::new(None));
        let el = Capture {
            child: opts.cx.contracts.build.element(child, parent, opts),
            id: id.clone(),
        };
        let mut root = div()
            .block()
            .w(px(width))
            .child(el.into_hooked())
            .into_any_element();
        root.layout_as_root_unrounded(
            size(
                AvailableSpace::Definite(px(width)),
                AvailableSpace::MaxContent,
            ),
            window,
            cx,
        );
        let block_size = f32::from(window.layout_size_unrounded(id.get().unwrap()).height);

        let mut style = box_style(parent, &child.style, opts.cx.vcx());
        let constrained =
            !matches!(style.height, None | Some(Len::Auto)) || style.min_height.is_some();
        let (content_width, border_size, start_edge) =
            shell(child, &style, width, opts, window, cx);
        style.height = None;
        style.min_height = None;
        style.max_height = None;
        let (_, edges, _) = shell(child, &style, width, opts, window, cx);
        SpannerMetrics {
            block_size,
            content_width,
            content_size: constrained.then_some((border_size - edges).max(0.)),
            start_edge,
            end_edge: (edges - start_edge).max(0.),
        }
    })
}

fn shell(
    child: &Element,
    style: &BoxStyle,
    width: f32,
    opts: &RenderOpts,
    window: &mut Window,
    cx: &mut App,
) -> (f32, f32, f32) {
    let id = Rc::new(Cell::new(None));
    let inner = Rc::new(Cell::new(None));
    let marker = Capture {
        child: div().w_full().h(px(0.)).into_any_element(),
        id: inner.clone(),
    };
    let el = opts
        .cx
        .contracts
        .build
        .styled_box(child, style, &opts.cx)
        .block()
        .child(marker.into_hooked());
    let el = Capture {
        child: el.into_any_element(),
        id: id.clone(),
    };
    let mut root = div()
        .block()
        .w(px(width))
        .child(el.into_hooked())
        .into_any_element();
    root.layout_as_root_unrounded(
        size(
            AvailableSpace::Definite(px(width)),
            AvailableSpace::MaxContent,
        ),
        window,
        cx,
    );
    let (id, inner) = (id.get().unwrap(), inner.get().unwrap());
    (
        f32::from(window.layout_size_unrounded(inner).width),
        f32::from(window.layout_size_unrounded(id).height),
        f32::from(window.layout_origin_unrounded(inner).y - window.layout_origin_unrounded(id).y),
    )
}

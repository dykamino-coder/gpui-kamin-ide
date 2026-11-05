//! Native-геометрия предка спаннера: ширина content box и раздельные кромки.
//! Щупы не публикуют LayoutId за пределами вложенного layout tree.

use crate::dom::Element;
use crate::hooks::{IntoHooked, Wrap};
use crate::layout::block::sizing::BoxDimensions;
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
    async fn request_layout_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> LayoutId {
        let id = self.child.request_layout_async(window, cx, stack).await;
        self.id.set(Some(id));
        id
    }
}

pub(super) async fn measure_async(
    child: &Element,
    parent: &BoxStyle,
    width: f32,
    opts: &RenderOpts,
    window: &mut Window,
    cx: &mut App,
    stack: &mut gpui::PaintStack,
) -> SpannerMetrics {
    window
        .with_nested_layout_async(async |window| {
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
            root.layout_as_root_unrounded_async(
                size(
                    AvailableSpace::Definite(px(width)),
                    AvailableSpace::MaxContent,
                ),
                window,
                cx,
                stack,
            )
            .await;
            let block_size = f32::from(window.layout_size_unrounded(id.get().unwrap()).height);

            let style = box_style(parent, &child.style, opts.cx.vcx());
            let sizes = BoxDimensions::for_element(child, &style);
            let constrained =
                !matches!(sizes.preferred[1], None | Some(Len::Auto)) || sizes.minimum[1].is_some();
            let (content_width, border_size, start_edge) =
                shell_async(child, &style, sizes, width, opts, window, cx, stack).await;
            let (_, edges, _) = shell_async(
                child,
                &style,
                sizes.unconstrained_height(),
                width,
                opts,
                window,
                cx,
                stack,
            )
            .await;
            SpannerMetrics {
                block_size,
                content_width,
                content_size: constrained.then_some((border_size - edges).max(0.)),
                start_edge,
                end_edge: (edges - start_edge).max(0.),
            }
        })
        .await
}

#[allow(clippy::too_many_arguments)]
async fn shell_async(
    child: &Element,
    style: &BoxStyle,
    sizes: BoxDimensions,
    width: f32,
    opts: &RenderOpts,
    window: &mut Window,
    cx: &mut App,
    stack: &mut gpui::PaintStack,
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
        .sized_box(child, style, &opts.cx, sizes)
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
    root.layout_as_root_unrounded_async(
        size(
            AvailableSpace::Definite(px(width)),
            AvailableSpace::MaxContent,
        ),
        window,
        cx,
        stack,
    )
    .await;
    let (id, inner) = (id.get().unwrap(), inner.get().unwrap());
    (
        f32::from(window.layout_size_unrounded(inner).width),
        f32::from(window.layout_size_unrounded(id).height),
        f32::from(window.layout_origin_unrounded(inner).y - window.layout_origin_unrounded(id).y),
    )
}

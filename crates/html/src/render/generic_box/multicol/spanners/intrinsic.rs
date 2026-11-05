//! Внутренний размер spanner path с исходным процентным содержащим блоком.

use super::{is_span, probe};
use crate::dom::{Element, Node};
use crate::layout::multicol::spanner::hoist_spanners_async;
use crate::layout::options::RenderOpts;
use crate::render::generic_box::columns::column_plan;
use crate::style::computed::BoxStyle;
use crate::style::values::value::Len;
use gpui::{App, AvailableSpace, IntoElement, ParentElement, Window, div};

pub(super) async fn measure_async(
    e: &Element,
    style: &BoxStyle,
    opts: &RenderOpts,
    available: gpui::Size<AvailableSpace>,
    window: &mut Window,
    cx: &mut App,
    stack: &mut gpui::PaintStack,
) -> f32 {
    let mut opts = opts.clone();
    opts.frag.watch = None;
    opts.cx.position.state = Default::default();
    opts.cx.deferred = Default::default();
    let opts = &opts;
    let mut e = e.clone();
    e.children =
        crate::layout::fragment::flow::column_percentages(&e.children, style, None, opts).into();
    let mut root = div()
        .children(opts.cx.contracts.build.blocks(&e.children, style, opts))
        .into_any_element();
    let width = f32::from(
        root.layout_as_root_unrounded_async(available, window, cx, stack)
            .await
            .width,
    );
    let mut measure =
        async |child: &Element, parent: &BoxStyle, width: f32, stack: &mut gpui::PaintStack| {
            probe::measure_async(child, parent, width, opts, window, cx, stack).await
        };
    if let Some(kids) =
        hoist_spanners_async(&e.children, style, opts, width, &mut measure, stack).await
    {
        e.children = kids.into();
    }
    let p = column_plan(&e, style, opts);
    let mut body = 0f32;
    let mut span = 0f32;
    for chunk in e.children.split_inclusive(is_span) {
        let (nodes, spanner) = match chunk.split_last() {
            Some((last, head)) if is_span(last) => (head, Some(last)),
            _ => (chunk, None),
        };
        let mut el = div()
            .children(opts.cx.contracts.build.blocks(nodes, style, opts))
            .into_any_element();
        body = body.max(f32::from(
            el.layout_as_root_unrounded_async(available, window, cx, stack)
                .await
                .width,
        ));
        if let Some(Node::Element(sp)) = spanner {
            let mut el = opts.cx.contracts.build.element(sp, style, opts);
            span = span.max(f32::from(
                el.layout_as_root_unrounded_async(available, window, cx, stack)
                    .await
                    .width,
            ));
        }
    }
    let n = f32::from(p.used_count.unwrap_or(1).max(1));
    let gap = p.used_gap * (n - 1.);
    let width = match (available.width, p.column_width) {
        (AvailableSpace::MinContent, Some(Len::Px(w))) => body.min(w),
        (_, Some(Len::Px(w))) => body.max(w) * n + gap,
        _ => body * n + gap,
    };
    width.max(span)
}

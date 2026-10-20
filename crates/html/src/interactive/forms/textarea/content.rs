//! Measure intrinsic keyword axes using the same paragraph as the editor paint.
use crate::style::values::value::Len;
use crate::{dom::Node, layout::options::RenderOpts, style::computed::Computed};
use gpui::{AvailableSpace, px, size};
use std::rc::Rc;

pub(super) fn content_measure(
    nodes: Vec<Node>,
    style: &Computed,
    opts: &RenderOpts,
    natural: [f32; 2],
    gutter: [f32; 2],
) -> Option<crate::layout::replaced::ControlMeasure> {
    let intrinsic = |v| matches!(v, Some(Len::MinContent | Len::MaxContent | Len::FitContent));
    let axes = [intrinsic(style.width), intrinsic(style.height)];
    if !axes[0] && !axes[1] {
        return None;
    }
    let style = style.clone();
    let opts = opts.clone();
    Some(Rc::new(move |known, available, window, cx| {
        let mut spaces = available;
        let extent = |known: Option<gpui::Pixels>, natural, gutter| {
            AvailableSpace::Definite(known.unwrap_or(px(natural)) - px(gutter))
        };
        if !axes[0] || known.width.is_some() {
            spaces.width = extent(known.width, natural[0], gutter[0]);
        }
        if !axes[1] || known.height.is_some() {
            spaces.height = extent(known.height, natural[1], gutter[1]);
        }
        let measured = window.with_nested_layout(|window| {
            let mut body = opts.cx.build.paragraph(&nodes, &style, &opts);
            let id = body.request_layout(window, cx);
            window.compute_layout(id, spaces, cx);
            window.layout_size_unrounded(id)
        });
        size(
            if axes[0] {
                measured.width + px(gutter[0])
            } else {
                px(natural[0])
            },
            if axes[1] {
                measured.height + px(gutter[1])
            } else {
                px(natural[1])
            },
        )
    }))
}

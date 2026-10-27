//! Доступные размеры детей: CSS-вход или явные размеры продолжения.

use crate::dom::Node;
use crate::layout::block::{available_height, available_width, sizing::BoxDimensions};
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use crate::style::values::value::Len;
use gpui::AnyElement;

pub(crate) async fn blocks_stack(
    stk: &mut reblessive::Stk,
    nodes: &[Node],
    inherited: &BoxStyle,
    opts: &RenderOpts,
) -> Vec<AnyElement> {
    super::blocks_stack_with_dimensions(
        stk,
        nodes,
        inherited,
        BoxDimensions::from_computed(inherited),
        opts,
    )
    .await
}

pub(super) fn child_options(
    inherited: &BoxStyle,
    sizes: &BoxDimensions,
    opts: &RenderOpts,
) -> Box<RenderOpts> {
    let outer = &opts.cx.constraints;
    let cb_width = match sizes.preferred[0] {
        Some(Len::Px(w)) if w > 0.0 => Some(w),
        _ => outer.cb_width,
    };
    let avail_h = available_height::inner_with_dimensions(
        inherited,
        sizes,
        outer.avail_h,
        outer.avail_w,
        opts.viewport.1,
    );
    let avail_w = available_width::inner_with_dimensions(inherited, sizes, outer.avail_w);
    opts.inner(|cx| {
        cx.constraints.cb_width = cb_width;
        cx.constraints.avail_w = avail_w;
        cx.constraints.avail_h = avail_h;
        cx.constraints.cell_bfc = false;
    })
}

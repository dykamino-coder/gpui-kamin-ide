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
    let sizes = BoxDimensions::from_computed(inherited);
    super::stack(stk, nodes, inherited, sizes, opts, false).await
}

pub(crate) async fn blocks_stack_with_dimensions(
    stk: &mut reblessive::Stk,
    nodes: &[Node],
    inherited: &BoxStyle,
    sizes: BoxDimensions,
    opts: &RenderOpts,
) -> Vec<AnyElement> {
    super::stack(stk, nodes, inherited, sizes, opts, false).await
}

/// Дети блока вне общего стека сборки: свой стек и свои факты длин (а вне
/// рендера — и поддеревьев). Сброс фактов едет в той же копии опций, что и
/// размеры детей.
pub(crate) fn blocks(nodes: &[Node], inherited: &BoxStyle, opts: &RenderOpts) -> Vec<AnyElement> {
    let sizes = BoxDimensions::from_computed(inherited);
    reblessive::Stack::new()
        .enter(|stk| super::stack(stk, nodes, inherited, sizes, opts, true))
        .finish()
}

pub(super) fn child_options(
    inherited: &BoxStyle,
    sizes: &BoxDimensions,
    opts: &RenderOpts,
    fresh: bool,
) -> RenderOpts {
    let outer = &opts.cx.constraints;
    // A child inherits the outer widths by copy; reading them here would
    // record a read of an outside width (`BoxConstraints::track_widths`).
    let cb_width = match sizes.preferred[0] {
        Some(Len::Px(w)) if w > 0.0 => Some(w),
        _ => None,
    };
    let avail_h = outer.with_avail_w(|avail_w| {
        available_height::inner_with_dimensions(
            inherited,
            sizes,
            outer.avail_h,
            avail_w,
            opts.viewport.1,
        )
    });
    opts.inner_value(|cx| {
        if fresh {
            cx.contracts.lengths = Default::default();
            if !cx.contracts.in_render {
                cx.contracts.subtrees = Default::default();
            }
        }
        if cb_width.is_some() {
            cx.constraints.set_cb_width(cb_width);
        }
        cx.constraints.derive_avail_w(|outer| {
            available_width::inner_with_dimensions(inherited, sizes, outer)
        });
        cx.constraints.avail_h = avail_h;
        cx.constraints.cell_bfc = false;
    })
}

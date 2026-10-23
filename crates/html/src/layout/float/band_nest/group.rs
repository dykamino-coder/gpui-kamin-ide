//! Группы paint среди вложенных блоков потока общего плана band: эффекты блока
//! охватывают его со всеми потомками, а float-ы потомков остаются в exclusions
//! хоста.

use crate::dom::{Element, Node};
use crate::style::computed::{BoxStyle, Computed};

/// The block is a paint group: a stacking context (CSS 2.1 Appendix E;
/// CSS Color 4 §opacity, CSS Transforms 1 §transform-rendering) or a mask or
/// clip that applies to the box and its descendants as one image
/// (CSS Masking 1 §clipping-paths, §the-mask).
pub(crate) fn paint_group(c: &Computed) -> bool {
    crate::style::computed::stacking::stacking_context(c)
        || c.mask_image.is_some()
        || c.clip_shape.is_some()
        || c.clip_inset.is_some()
        || c.clip_polygon.is_some()
        || c.clip_edges.is_some()
        || c.clip_xywh.is_some()
        || c.clip_ref.is_some()
        || c.filter_ref.is_some()
        || c.backdrop_blur.is_some()
}

/// An absolutely or fixed positioned descendant at any depth.
pub(super) fn positioned_inside(nodes: &[Node]) -> bool {
    nodes.iter().any(|n| match n {
        Node::Element(e) => {
            matches!(
                e.style.used().position,
                Some(crate::style::computed::Position::Absolute)
                    | Some(crate::style::computed::Position::Fixed)
            ) || positioned_inside(&e.children)
        }
        Node::Text(_) => false,
    })
}

/// The effects of paint group `c` around a container of its border box:
/// the same chain as a block in the flow (`position_element`) — the folded
/// shift and opacity on the box, then its transform, then the grouped
/// effects. The shell of the box carries none of them (`group_shell`).
pub(super) fn group_wrap(
    c: &Element,
    inherited: &BoxStyle,
    cx: &crate::layout::cx::LayoutCx,
) -> crate::layout::float::band_flow::GroupWrap {
    let (c, inherited, values, build) = (c.clone(), inherited.clone(), cx.vcx().keep(), cx.build);
    std::rc::Rc::new(move |container: gpui::Div| {
        build.paint_group(container, &c, &inherited, values.vcx())
    })
}

/// The box shell of paint group `c`: its group effects move to the container
/// around it and its descendants (`group_wrap`).
pub(crate) fn group_shell(c: &mut Computed) {
    c.opacity = None;
    c.translate = None;
    c.transform = None;
    c.rotate_prop = None;
    c.scale_prop = None;
}

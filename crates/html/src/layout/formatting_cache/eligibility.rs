//! Отказ от reuse при внешних рекордерах и сложных форматирующих контекстах.

use crate::dom::{Element, Node};
use crate::layout::options::RenderOpts;
use crate::style::computed::Position;

pub(crate) fn eligible(source: &Element, opts: &RenderOpts) -> bool {
    let cx = &opts.cx;
    if !cx.doc().values.resources_unchanged()
        || cx.clamp.input.is_some()
        || cx.clamp.frame.is_some()
        || cx.clamp.atom_depth != 0
        || cx.clamp.paragraph.is_some()
        || cx.scroll.frame.is_some()
        || cx.scroll.target.is_some()
        || cx.scroll.sticky.is_some()
        || cx.position.oof_identities.is_some()
        || cx.position.geometry_record.is_some()
        || cx.position.geometry.is_some()
        || cx.position.row_geometry.is_some()
        || cx.position.continuation_slots.is_some()
        || cx.position.oof_sizes.is_some()
        || cx.position.oof_computed.is_some()
        || cx.position.oof_trial.is_some()
        || cx.position.line_box.is_some()
        || cx.position.inline_cb.is_some()
        || cx.fragmentation.lines.is_some()
        || cx.inline.inline_measure.is_some()
        || cx.inline.pure_atom_paragraph
        || cx.inline.outside_marker_rtl.is_some()
        || !cx.inline.trim_requests.is_empty()
        || cx.inline.trim_para.is_some()
        || cx.ruby.base.is_some()
        || cx.ruby.layers.is_some()
        || cx.ruby.extents.is_some()
        || opts.frag.in_stack_copy
        || opts.frag.outer_space.is_some()
        || opts.frag.watch.is_some()
        || !opts.frag.float_struts.is_empty()
        || !opts.frag.float_floors.is_empty()
        || opts.frag.paged.is_some()
    {
        return false;
    }
    let mut pending = vec![source];
    while let Some(element) = pending.pop() {
        let style = element.style.used();
        // Эти боксы не создают внешних побочных эффектов layout.
        // Числовой reuse допускает их float и фрагментацию по колонкам.
        if !(element.marks.shape_flow.is_some()
            || matches!(
                element.tag.as_str(),
                "div"
                    | "p"
                    | "span"
                    | "b"
                    | "i"
                    | "em"
                    | "strong"
                    | "h1"
                    | "h2"
                    | "h3"
                    | "h4"
                    | "h5"
                    | "h6"
            ))
            || element.anim.is_some()
            || element.hover.is_some()
            || element.first_letter.is_some()
            || element.first_line.is_some()
            || element.list_item.is_some()
            || !matches!(
                style.reset_box_layout.display,
                None | Some(crate::style::computed::Display::Block)
            )
            || !matches!(
                style.reset_box_layout.overflow_x,
                None | Some(crate::style::computed::Overflow::Visible)
            )
            || !matches!(
                style.reset_box_layout.overflow_y,
                None | Some(crate::style::computed::Overflow::Visible)
            )
            || style.reset_transforms.transform.is_some()
            || style.reset_transforms.perspective.is_some()
            || style.reset_transforms.translate.is_some()
            || style.reset_transforms.offset_path.is_some()
            || style.reset_transforms.rotate_prop.is_some()
            || style.reset_transforms.scale_prop.is_some()
            || style.reset_transforms.preserve_3d == Some(true)
            || style.reset_effects.filter.is_some()
            || style.reset_effects.filter_ref.is_some()
            || !matches!(
                style.reset_positioned.position,
                None | Some(Position::Static)
            )
            || style.reset_positioned.anchor_name.is_some()
            || style.wm().vertical()
        {
            return false;
        }
        pending.extend(element.children.iter().filter_map(|node| match node {
            Node::Element(child) => Some(child),
            _ => None,
        }));
    }
    true
}

//! Блок из одних флоатов: `zero_float_blocks` помечает его нулевой высотой (CSS 2.1 §10.6.3:
//! флоаты в высоту содержимого обычного блока не входят); `used_style` отдаёт её только нативной
//! коробке, вычисленная остаётся `auto` ради неопределённой базы долей (§10.5).

use crate::document::state::ValueCx;
use crate::dom::whitespace::blank_text;
use crate::dom::{Element, Node};
use crate::layout::block::struts::zero_len;
use crate::layout::box_kind::{
    in_flow, inline_level_box, out_of_flow, own_context, replaced_inline,
};
use crate::style::cascade::materialize::box_style;
use crate::style::computed::{BoxStyle, Display};
use crate::style::values::value::Len;

/// CSS 2.1 §10.6.3 ignores floats in ordinary blocks' auto content height.
/// Borders and padding stop margin collapse but do not make floats in-flow.
/// Prove only zero-height block content; unknown line boxes stay on layout.
fn zero_inflow(e: &Element, inherited: &BoxStyle, vcx: ValueCx<'_>) -> bool {
    let st = e.style.computed();
    if !in_flow(&st)
        || own_context(e)
        || inline_level_box(e)
        || replaced_inline(&e.tag)
        || !matches!(st.display, None | Some(Display::Block))
        || e.list_item.is_some()
        || matches!(e.tag.as_str(), "html" | "body")
        || !matches!(st.height, None | Some(Len::Auto))
        || !zero_len(st.min_height)
        || e.style.cascaded().keep_spaces == Some(true)
        || e.style.cascaded().preserve_newlines == Some(true)
    {
        return false;
    }
    let merged = box_style(inherited, &e.style, vcx);
    !merged.wm().vertical() && e.children.iter().all(|node| zero_child(node, &merged, vcx))
}

fn zero_child(node: &Node, inherited: &BoxStyle, vcx: ValueCx<'_>) -> bool {
    let e = match node {
        Node::Text(text) => {
            return blank_text(text)
                && inherited.keep_spaces != Some(true)
                && inherited.preserve_newlines != Some(true);
        }
        // Only floats and absolutes leave the flow (CSS 2.1 §9.3). An
        // inline-block/-flex/-grid is inline-level content in flow: it sits in
        // a line box, which the auto height counts (§10.6.3). `in_flow` is the
        // block-stack predicate and rejects those too.
        Node::Element(e) if out_of_flow(&e.style.computed()) => return true,
        Node::Element(e) => e,
    };
    let st = e.style.computed();
    if st.display == Some(Display::None) {
        return true;
    }
    let b = st.borders();
    let merged = box_style(inherited, &e.style, vcx);
    !merged.wm().vertical()
        && !inline_level_box(e)
        && matches!(st.display, None | Some(Display::Block))
        && e.list_item.is_none()
        && !own_context(e)
        && !replaced_inline(&e.tag)
        && st.clear.is_none()
        && matches!(st.height, None | Some(Len::Auto) | Some(Len::Px(0.0)))
        && zero_len(st.min_height)
        && [
            st.margin.top,
            st.margin.bottom,
            st.padding.top,
            st.padding.bottom,
            b.top,
            b.bottom,
        ]
        .into_iter()
        .all(zero_len)
        && e.style.cascaded().keep_spaces != Some(true)
        && e.style.cascaded().preserve_newlines != Some(true)
        && e.children.iter().all(|node| zero_child(node, &merged, vcx))
}

/// Apply the zero-content proof with the real parent's inherited white-space.
pub(crate) fn zero_float_blocks(nodes: &mut [Node], inherited: &BoxStyle, vcx: ValueCx<'_>) {
    if matches!(
        inherited.position,
        Some(crate::style::computed::Position::Absolute)
            | Some(crate::style::computed::Position::Fixed)
    ) {
        return;
    }
    for node in nodes {
        let Node::Element(e) = node else { continue };
        let has_float = e.children.iter().any(|node| {
            matches!(node,
            Node::Element(child) if child.style.computed().float.is_some_and(|side| side != 0))
        });
        // Our float host carries floats to later boxes and to the formatting
        // root only through this block's height (see `dom::float_tail`).
        if has_float && e.style.computed().float_tail && zero_inflow(e, inherited, vcx) {
            e.marks.auto_zero_height = true;
        }
    }
}

/// Keep the computed height auto: percentages still need an indefinite basis
/// (CSS 2.1 §10.5). Only the native box receives the proven used height.
pub(crate) fn used_style(e: &Element, style: &BoxStyle) -> Option<BoxStyle> {
    if !e.marks.auto_zero_height
        || style.wm().vertical()
        || !matches!(style.height, None | Some(Len::Auto))
    {
        return None;
    }
    let mut used = style.clone();
    used.height = Some(Len::Px(0.0));
    Some(used)
}

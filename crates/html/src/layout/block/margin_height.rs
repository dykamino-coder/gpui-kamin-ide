//! Доказуемые высоты блоков при схлопывании полей: когда высота не равна высоте содержимого,
//! поля ребёнка и родителя не сливаются (CSS 2.2 §10.7, Blink `block_layout_algorithm.cc`).
//! `lowers` — `max-height` режет единственного определённого ребёнка; `raises` — `min-height`
//! тянет коробку выше содержимого; `separate` обнуляет нижние поля детей. `zero_float_blocks`
//! помечает блок из одних флоатов нулевой высотой (CSS 2.1 §10.6.3); `used_style` отдаёт её
//! только нативной коробке, вычисленная остаётся `auto` ради неопределённой базы долей (§10.5).

use crate::document::state::ValueCx;
use crate::dom::{Element, Node};
use crate::layout::block::struts::{margin_or_bail, margin_px, pin_inherited_margins, zero_len};
use crate::layout::box_kind::{
    in_flow, inline_level_box, out_of_flow, own_context, replaced_inline,
};
use crate::style::cascade::materialize::box_style;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use crate::text::text_box::blank_text;

pub(super) fn separate(e: &mut Element, vcx: crate::document::state::ValueCx<'_>) -> bool {
    if !lowers(e, vcx) {
        return false;
    }
    // The child's end margin cannot change this constrained parent's used
    // height. Leaving a negative margin in the flex adapter would shrink the
    // auto height again, undoing the constraint that separated the margins.
    for node in &mut e.children {
        if let Node::Element(child) = node
            && in_flow(&child.style.computed())
        {
            pin_inherited_margins(child, false, true, vcx);
            child.style.used_mut().margin.bottom = Some(Len::Px(0.0));
        }
    }
    true
}

/// CSS 2.2 §10.7 reruns height and margin calculation with the limiting
/// height. Blink block_layout_algorithm.cc:1459-1466 likewise separates
/// the end margin when the used block size differs from the intrinsic size.
/// Unknown content remains on the existing path; this proof uses only
/// one definite block child whose border edge can be calculated here.
pub(super) fn lowers(e: &Element, vcx: crate::document::state::ValueCx<'_>) -> bool {
    let Some(mut cap) = margin_px(e.style.computed().max_height, &e.style.computed(), vcx) else {
        return false;
    };
    let edges = |style: &Computed| -> Option<f32> {
        let b = style.borders();
        [style.padding.top, style.padding.bottom, b.top, b.bottom]
            .into_iter()
            .try_fold(0.0, |sum, value| match value {
                None => Some(sum),
                Some(Len::Px(v)) => Some(sum + v),
                _ => None,
            })
    };
    if e.style.computed().border_box == Some(true) {
        let Some(inset) = edges(&e.style.computed()) else {
            return false;
        };
        cap = (cap - inset).max(0.0);
    }
    if let Some(min) = margin_px(e.style.computed().min_height, &e.style.computed(), vcx) {
        cap = cap.max(min);
    } else if !zero_len(e.style.computed().min_height) {
        return false;
    }
    let mut bottom = None;
    for node in &e.children {
        let child = match node {
            Node::Text(text) if blank_text(text) => continue,
            Node::Text(_) => return false,
            Node::Element(child) if !in_flow(&child.style.computed()) => continue,
            Node::Element(child) => child,
        };
        if bottom.is_some()
            || inline_level_box(child)
            || child.style.computed().clear.is_some()
            || child.style.computed().min_height.is_some()
            || child.style.computed().max_height.is_some()
        {
            return false;
        }
        let Some(height) = margin_px(child.style.computed().height, &child.style.computed(), vcx)
        else {
            return false;
        };
        let Some(inset) = edges(&child.style.computed()) else {
            return false;
        };
        let Some(top) = margin_or_bail(
            child.style.computed().margin.top,
            &child.style.computed(),
            vcx,
        ) else {
            return false;
        };
        let height = if child.style.computed().border_box == Some(true) {
            height.max(inset)
        } else {
            height + inset
        };
        bottom = Some(top + height);
    }
    bottom.is_some_and(|bottom| bottom > cap)
}

pub(super) fn raises(e: &Element, vcx: crate::document::state::ValueCx<'_>) -> bool {
    let zero = |l: Option<Len>| matches!(l, None | Some(Len::Px(0.0)) | Some(Len::Pct(0.0)));
    match margin_px(e.style.computed().min_height, &e.style.computed(), vcx) {
        None => !zero(e.style.computed().min_height),
        Some(mh) if mh <= 0.0 => false,
        Some(mh) => {
            let mut sum = 0.0f32;
            let mut known = true;
            for c in &e.children {
                match c {
                    Node::Text(t) if blank_text(t) => {}
                    Node::Text(_) => known = false,
                    Node::Element(ch) if !in_flow(&ch.style.computed()) => {}
                    Node::Element(ch) if ch.inline && inline_level_box(ch) => known = false,
                    Node::Element(ch) => {
                        let b = ch.style.computed().borders();
                        let side = |l: Option<Len>| match l {
                            None => Some(0.0),
                            Some(Len::Px(v)) => Some(v),
                            _ => None,
                        };
                        let own = match (
                            margin_px(ch.style.computed().height, &ch.style.computed(), vcx),
                            side(ch.style.computed().padding.top),
                            side(ch.style.computed().padding.bottom),
                            side(b.top),
                            side(b.bottom),
                        ) {
                            (Some(h), Some(pt), Some(pb), Some(bt), Some(bb)) => {
                                Some(h + pt + pb + bt + bb)
                            }
                            _ => None,
                        };
                        match own {
                            Some(v) => sum += v,
                            None => known = false,
                        }
                    }
                }
                if !known {
                    break;
                }
            }
            !known || mh > sum + 0.01
        }
    }
}

/// CSS 2.1 §10.6.3 ignores floats in ordinary blocks' auto content height.
/// Borders and padding stop margin collapse but do not make floats in-flow.
/// Prove only zero-height block content; unknown line boxes stay on layout.
fn zero_inflow(
    e: &Element,
    inherited: &crate::style::computed::BoxStyle,
    vcx: ValueCx<'_>,
) -> bool {
    if !in_flow(&e.style.computed())
        || own_context(e)
        || inline_level_box(e)
        || replaced_inline(&e.tag)
        || !matches!(e.style.computed().display, None | Some(Display::Block))
        || e.list_item.is_some()
        || matches!(e.tag.as_str(), "html" | "body")
        || !matches!(e.style.computed().height, None | Some(Len::Auto))
        || !zero_len(e.style.computed().min_height)
        || e.style.cascaded().keep_spaces == Some(true)
        || e.style.cascaded().preserve_newlines == Some(true)
    {
        return false;
    }
    let merged = box_style(inherited, &e.style, vcx);
    !merged.wm().vertical() && e.children.iter().all(|node| zero_child(node, &merged, vcx))
}

fn zero_child(node: &Node, inherited: &crate::style::computed::BoxStyle, vcx: ValueCx<'_>) -> bool {
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
    if e.style.computed().display == Some(Display::None) {
        return true;
    }
    let b = e.style.computed().borders();
    let merged = box_style(inherited, &e.style, vcx);
    !merged.wm().vertical()
        && !inline_level_box(e)
        && matches!(e.style.computed().display, None | Some(Display::Block))
        && e.list_item.is_none()
        && !own_context(e)
        && !replaced_inline(&e.tag)
        && e.style.computed().clear.is_none()
        && matches!(
            e.style.computed().height,
            None | Some(Len::Auto) | Some(Len::Px(0.0))
        )
        && zero_len(e.style.computed().min_height)
        && [
            e.style.computed().margin.top,
            e.style.computed().margin.bottom,
            e.style.computed().padding.top,
            e.style.computed().padding.bottom,
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
pub(crate) fn zero_float_blocks(
    nodes: &mut [Node],
    inherited: &crate::style::computed::BoxStyle,
    vcx: ValueCx<'_>,
) {
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
pub(crate) fn used_style(
    e: &Element,
    style: &crate::style::computed::BoxStyle,
) -> Option<crate::style::computed::BoxStyle> {
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

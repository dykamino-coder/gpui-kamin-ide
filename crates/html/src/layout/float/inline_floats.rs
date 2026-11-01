//! Подъём флоатов из неоформленных строчных контейнеров в раскладку флоатов их блока
//! (перед `wrap_floats`): прозрачный `<span>` рвётся на фрагменты вокруг каждого флоата,
//! текст по обе стороны сохраняет унаследованный стиль строчного, а содержащим блоком
//! флоата остаётся блок (CSS 2 §9.5, §10.1). Последовательность `nowrap` не рвётся
//! (CSS Text §5.1); ортогональные флоаты и руби остаются в своём строчном.
//! `transparent` — общий предикат неоформленного строчного для соседних проходов.

use crate::dom::{Element, Node};
use crate::style::cascade::materialize::box_style;
use crate::style::computed::BoxStyle;
use crate::values::ValueCx;

pub(super) fn lift(nodes: Vec<Node>, parent: &BoxStyle, vcx: ValueCx<'_>) -> Vec<Node> {
    if parent.wm().vertical() {
        return nodes;
    }
    nodes
        .into_iter()
        .flat_map(|node| split(node, parent, vcx))
        .collect()
}

pub(crate) fn transparent(e: &Element) -> bool {
    transparent_with_clear(e, true)
}

pub(crate) fn transparent_with_clear(e: &Element, clearance_possible: bool) -> bool {
    let s = &e.style.used();
    let zero = |v| matches!(v, None | Some(crate::style::values::value::Len::Px(0.0)));
    let undecorated = [
        s.reset_box_layout.margin,
        s.reset_box_layout.padding,
        s.borders(),
    ]
    .iter()
    .all(|s| [s.top, s.right, s.bottom, s.left].into_iter().all(zero));
    ((e.inline && s.reset_box_layout.display.is_none()) || s.reset_context.inline_display == Some(true))
        // css-ruby-1 §2: ruby boxes pair bases with annotations; splitting
        // them around a float would break that pairing.
        && !matches!(e.tag.as_str(), "ruby" | "rb" | "rbc" | "rt" | "rtc" | "rp")
        && s.reset_box_layout.float.is_none_or(|f| f == 0)
        && (!clearance_possible || s.reset_box_layout.clear.is_none())
        && s.reset_positioned.position.is_none()
        && e.hover.is_none()
        && e.first_letter.is_none()
        && e.first_line.is_none()
        && undecorated
        && s.reset_background.background.is_none()
        && s.reset_background.bg_image.is_none()
        && s.reset_background.gradient.is_none()
        && s.reset_effects.opacity.is_none()
        && s.reset_effects.filter.is_none()
        && s.reset_transforms.transform.is_none()
        && s.reset_effects.blend.is_none()
        && s.reset_mask.clip_polygon.is_none()
        && s.reset_mask.mask_image.is_none()
        && s.reset_border.outline.is_none()
        && s.reset_effects.shadows.is_empty()
        && s.reset_effects.shadow_raw.is_none()
        && s.reset_cascade.bidi_override.is_none()
        && s.reset_cascade.bidi_isolate.is_none()
        && s.reset_cascade.bidi_embed.is_none()
        && s.reset_cascade.bidi_plaintext.is_none()
        && e.style.own_vertical_align().is_none()
        && s.reset_text_box.vertical_shift.is_none()
        && s.reset_text_box.vertical_shift_len.is_none()
        // Своё письмо и направление — объявленные: вычисленные наследуются.
        && e.style.cascaded().inh.writing.writing_mode.is_none()
        && !s.reset_text_box.turned_line
        && e.style.cascaded().inh.writing.rtl.is_none()
}

fn split(node: Node, parent: &BoxStyle, vcx: ValueCx<'_>) -> Vec<Node> {
    let Node::Element(e) = &node else {
        return vec![node];
    };
    // CSS2 sections 9.2.2 and 9.5: a float-only inline still has an empty
    // in-flow fragment. Keep its margins there, while the floats belong to
    // the block's shared exclusion space, not an atomic paragraph child.
    let margin_fragment = if !transparent(e) {
        let mut bare = e.clone();
        bare.style.used_mut().reset_box_layout.margin = Default::default();
        transparent(&bare)
            && e.style
                .cascaded()
                .inh
                .text_flow
                .preserve_newlines
                .or(parent.inh.text_flow.preserve_newlines)
                != Some(true)
            && e.style
                .cascaded()
                .inh
                .text_flow
                .keep_spaces
                .or(parent.inh.text_flow.keep_spaces)
                != Some(true)
            && e.children
                .iter()
                .all(|n| crate::layout::box_kind::is_blank(n) || floated(n))
    } else {
        false
    };
    if (!transparent(e) && !margin_fragment) || !e.children.iter().any(contains_float) {
        return vec![node];
    }
    let style = std::rc::Rc::new(box_style(parent, &e.style, vcx));
    let children = lift(e.children.to_vec(), &style, vcx);
    if !children.iter().any(floated) {
        return vec![node];
    }
    // CSS 2.1 §9.5 and CSS Text §5.1: an out-of-flow float does not
    // split a nowrap text sequence. Keep text in one inline fragment so
    // float placement can defer it past the whole unbreakable sequence.
    // Leading floats and preserved hard breaks remain on their own path.
    let nowrap_text = (parent.inh.text_flow.nowrap != Some(true)
        && style.inh.text_flow.nowrap == Some(true)
        && style.inh.text_flow.preserve_newlines != Some(true)
        && children
            .iter()
            .all(|n| matches!(n, Node::Text(_)) || floated(n))
        && matches!(
            children.iter().find(|n| match n {
                Node::Text(t) => !crate::dom::whitespace::blank_text(t),
                _ => true,
            }),
            Some(Node::Text(_))
        ))
    .then(|| {
        children
            .iter()
            .filter_map(|n| match n {
                Node::Text(text) => Some(text.as_str()),
                _ => None,
            })
            .collect::<String>()
    });
    let fragment = |children: Vec<Node>| {
        let mut fragment = e.clone();
        fragment.children = children.into();
        Node::Element(fragment)
    };
    let mut output = Vec::new();
    let mut pending = Vec::new();
    if margin_fragment {
        output.push(fragment(Vec::new()));
    }
    for child in children {
        if margin_fragment && crate::layout::box_kind::is_blank(&child) {
            continue;
        }
        if floated(&child) {
            if !pending.is_empty() {
                output.push(fragment(std::mem::take(&mut pending)));
            }
            let Node::Element(mut float) = child else {
                // floated возвращает true только для Node::Element.
                unreachable!()
            };
            // CSS 2 sections 9.5 and 10.1: the block becomes the box parent
            // and stays the containing block; the element keeps its style and
            // the inline stays its parent in the element tree.
            float.set_moved_box(style.clone());
            output.push(Node::Element(float));
        } else {
            pending.push(child);
        }
    }
    if !pending.is_empty() {
        output.push(fragment(pending));
    }
    if let Some(text) = nowrap_text {
        let floats = output.into_iter().filter(floated);
        return std::iter::once(fragment(vec![Node::Text(text)]))
            .chain(floats)
            .collect();
    }
    output
}

/// Orthogonal floats stay in their inline: the inline-float host measures
/// their intrinsic size inside shrink-to-fit containers
/// (`orthogonal-writing-mode-float-in-inline`).
fn floated(node: &Node) -> bool {
    matches!(node, Node::Element(e) if e.style.used().reset_box_layout.float.is_some_and(|f| f != 0)
        && !e.style.cascaded().wm().vertical())
}

fn contains_float(node: &Node) -> bool {
    floated(node)
        || matches!(node, Node::Element(e) if transparent(e)
            && e.children.iter().any(contains_float))
}

#[cfg(test)]
mod tests;

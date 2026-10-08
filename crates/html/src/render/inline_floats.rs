//! Lift floats from undecorated inline containers into their block's float layout.
//! Text fragments retain the inline's inherited style on both sides of each float.

use super::{Computed, Element, Node, inline};

pub(super) fn lift(nodes: Vec<Node>, parent: &Computed) -> Vec<Node> {
    if parent.vertical == Some(true) {
        return nodes;
    }
    nodes
        .into_iter()
        .flat_map(|node| split(node, parent))
        .collect()
}

fn transparent(e: &Element) -> bool {
    let s = &e.style;
    let zero = |v| matches!(v, None | Some(crate::value::Len::Px(0.0)));
    let undecorated = [s.margin, s.padding, s.borders()]
        .iter()
        .all(|s| [s.top, s.right, s.bottom, s.left].into_iter().all(zero));
    ((e.inline && s.display.is_none()) || s.inline_display == Some(true))
        && !s.float.is_some_and(|f| f != 0)
        && s.clear.is_none()
        && s.position.is_none()
        && e.hover.is_none()
        && e.first_letter.is_none()
        && e.first_line.is_none()
        && undecorated
        && s.background.is_none()
        && s.bg_image.is_none()
        && s.gradient.is_none()
        && s.opacity.is_none()
        && s.filter.is_none()
        && s.transform.is_none()
        && s.blend.is_none()
        && s.clip_polygon.is_none()
        && s.mask_image.is_none()
        && s.outline.is_none()
        && s.shadows.is_empty()
        && s.shadow_raw.is_none()
        && s.bidi_override.is_none()
        && s.bidi_isolate.is_none()
        && s.bidi_embed.is_none()
        && s.bidi_plaintext.is_none()
        && s.vertical_align.is_none()
        && s.vertical_shift.is_none()
        && s.vertical_shift_len.is_none()
        && s.vertical.is_none()
        && s.rotated_line.is_none()
        && s.rtl.is_none()
}

fn split(node: Node, parent: &Computed) -> Vec<Node> {
    let Node::Element(e) = &node else {
        return vec![node];
    };
    if !transparent(e) || !e.children.iter().any(contains_float) {
        return vec![node];
    }
    let style = inline::inherit(parent, &e.style);
    let children = lift(e.children.clone(), &style);
    if !children.iter().any(floated) {
        return vec![node];
    }
    let fragment = |children: Vec<Node>| {
        let mut fragment = e.clone();
        fragment.children = children;
        Node::Element(fragment)
    };
    let mut output = Vec::new();
    let mut pending = Vec::new();
    for child in children {
        if floated(&child) {
            if !pending.is_empty() {
                output.push(fragment(std::mem::take(&mut pending)));
            }
            let Node::Element(mut float) = child else {
                unreachable!()
            };
            // CSS 2 sections 9.5 and 10.1: the block remains the containing
            // block, while inherited text properties come through the inline.
            let containing = inline::inherit(parent, &float.style);
            float.style = inline::inherit(&style, &float.style);
            float.style.cb_height_def = containing.cb_height_def;
            float.style.quirk_pct_base = containing.quirk_pct_base;
            output.push(Node::Element(float));
        } else {
            pending.push(child);
        }
    }
    if !pending.is_empty() {
        output.push(fragment(pending));
    }
    output
}

fn floated(node: &Node) -> bool {
    matches!(node, Node::Element(e) if e.style.float.is_some_and(|f| f != 0))
}

fn contains_float(node: &Node) -> bool {
    floated(node)
        || matches!(node, Node::Element(e) if transparent(e)
            && e.children.iter().any(contains_float))
}

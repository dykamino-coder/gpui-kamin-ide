//! Paint text shadows with the same paragraph geometry and decoration mask.

use super::{RenderOpts, gather_text, normalize_for_shadow, paragraph};
use crate::computed::{Computed, Shadow};
use crate::dom::Node;
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled, div, px};

fn shadow_layer(
    text: &str,
    shadow: &Shadow,
    nodes: &[Node],
    style: &Computed,
    opts: &RenderOpts,
    same_paragraph: bool,
) -> AnyElement {
    let copy = if same_paragraph {
        let mut mask = style.clone();
        mask.color = Some(shadow.color);
        mask.td_color = Some(shadow.color);
        for decor in &mut mask.decors {
            decor.color = shadow.color;
        }
        mask.text_shadow = None;
        mask.text_shadow_rest.clear();
        mask.text_shadow_raw = None;
        // CSS Text Decoration 3 §4: a shadow masks both glyphs and their
        // decorations. Reusing the paragraph keeps baseline, spacing and
        // decoration geometry identical; its color is the shadow's color.
        paragraph(nodes, &mask, opts)
    } else {
        div()
            .text_color(shadow.color.to_hsla())
            .child(SharedString::from(text.to_string()))
            .into_any_element()
    };
    let copy = if shadow.blur > 0.5 {
        let mut group = crate::interact::Grouped::new(copy);
        group.blur = shadow.blur * 0.5;
        group.into_any_element()
    } else {
        copy
    };
    // The offset belongs outside the blur buffer so its child has bounds.
    let mut placed = div().absolute().left(px(shadow.x)).top(px(shadow.y));
    if same_paragraph {
        placed = placed.w_full();
    }
    placed.child(copy).into_any_element()
}

pub(super) fn with_text_shadow(
    el: AnyElement,
    style: &Computed,
    nodes: &[Node],
    opts: &RenderOpts,
) -> AnyElement {
    let Some(shadow) = style.text_shadow else {
        return el;
    };
    let mut plain = String::new();
    gather_text(nodes, &mut plain);
    let plain = crate::inline::transform_case(&normalize_for_shadow(&plain), style);
    if plain.trim().is_empty() {
        return el;
    }
    // Mixed inline styles and rotated paragraphs retain their existing route.
    let same_paragraph = nodes.iter().all(|n| matches!(n, Node::Text(_)))
        && style.vertical != Some(true)
        && style.rotated_line != Some(true)
        && style.first_line.is_none()
        && style.first_letter.is_none()
        && style.text_emphasis.is_none();
    // CSS Text Decoration 3 §4: first shadow is on top, all below the text.
    let layers = style
        .text_shadow_rest
        .iter()
        .rev()
        .chain(std::iter::once(&shadow))
        .map(|s| shadow_layer(plain.trim(), s, nodes, style, opts, same_paragraph));
    div()
        .relative()
        .children(layers)
        .child(el)
        .into_any_element()
}

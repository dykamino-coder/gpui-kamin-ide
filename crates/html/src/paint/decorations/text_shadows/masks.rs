//! Маски цветовых групп для составных текстовых теней.

use super::{ShadowStyle, atomic};
use crate::dom::Node;
use crate::hooks::IntoHooked;
use crate::layout::options::RenderOpts;
use crate::style::computed::Shadow;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};

pub(super) fn transparent() -> crate::style::values::value::Color {
    crate::style::values::value::Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    }
}

/// Слой одной тени строчных коробок: тот же абзац (геометрия строк та же),
/// где текст коробок этой группы окрашен в цвет тени, а всё прочее —
/// прозрачно: чужой текст, фоны, рамки, контуры и атомы.
pub(super) fn masked_layer(
    group: &[Shadow],
    shadow: &Shadow,
    block: &ShadowStyle,
    nodes: &[Node],
    style: &crate::style::computed::BoxStyle,
    opts: &RenderOpts,
) -> AnyElement {
    let vcx = opts.cx.vcx();
    let on = block.resolved() == group;
    let ink = if on {
        shadow.used_color(style.current_color())
    } else {
        transparent()
    };
    let mut mask = style.clone();
    mask.text_shadow_ink = Some(ink);
    mask.text_shadow = None;
    mask.text_shadow_rest.clear();
    mask.text_shadow_raw = None;
    let masked = mask_nodes(nodes, block, group, shadow, opts.cx.vcx());
    let copy = opts.cx.build.paragraph(&masked, &mask, opts);
    let copy = if shadow.blur > 0.5 {
        let mut g = crate::paint::effects::grouped_element::Grouped::new(copy, vcx);
        g.blur = shadow.blur * 0.5;
        g.hooked()
    } else {
        copy
    };
    div()
        .absolute()
        .left(px(shadow.x))
        .top(px(shadow.y))
        .w_full()
        .child(copy)
        .into_any_element()
}

pub(super) fn mask_nodes(
    nodes: &[Node],
    parent: &ShadowStyle,
    group: &[Shadow],
    shadow: &Shadow,
    vcx: crate::document::state::ValueCx<'_>,
) -> Vec<Node> {
    nodes
        .iter()
        .map(|n| match n {
            Node::Text(_) => n.clone(),
            Node::Element(e) => {
                let mut e = e.clone();
                if atomic(&e, vcx) {
                    e.style.used_mut().hidden = Some(true);
                    return Node::Element(e);
                }
                let list = parent.child(&e);
                let ink = if list.resolved() == group {
                    shadow.used_color(e.style.cascaded().current_color())
                } else {
                    transparent()
                };
                let s = e.style.used_mut();
                s.text_shadow_ink = Some(ink);
                s.background = None;
                s.bg_image = None;
                s.shadows.clear();
                s.outline = None;
                s.border_colors = std::array::from_fn(|_| transparent().into());
                s.text_shadow = None;
                s.text_shadow_rest.clear();
                s.text_shadow_raw = None;
                e.children = mask_nodes(&e.children, &list, group, shadow, vcx);
                Node::Element(e)
            }
        })
        .collect()
}

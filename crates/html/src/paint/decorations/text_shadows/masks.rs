//! Маски цветовых групп для составных текстовых теней.

use super::{ShadowStyle, atomic};
use crate::dom::Node;
use crate::hooks::IntoHooked;
use crate::layout::options::RenderOpts;
use crate::style::computed::{BoxStyle, Shadow};
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
    style: &BoxStyle,
    opts: &RenderOpts,
) -> AnyElement {
    let frame = opts.cx.frame();
    let on = block.resolved() == group;
    let ink = if on {
        shadow.used_color(style.current_color())
    } else {
        transparent()
    };
    let mut mask = style.clone();
    mask.reset_text_box.text_shadow_ink = Some(ink);
    mask.inh.text_ink.text_shadow = None;
    mask.inh.text_ink.text_shadow_rest.clear();
    mask.reset_text_box.text_shadow_raw = None;
    let masked = mask_nodes(nodes, block, group, shadow, opts.cx.vcx());
    let copy = opts.cx.contracts.build.paragraph(&masked, &mask, opts);
    let copy = if shadow.blur > 0.5 {
        let mut g = crate::paint::effects::grouped_element::Grouped::new(copy, frame);
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
    vcx: crate::values::ValueCx<'_>,
) -> Vec<Node> {
    nodes
        .iter()
        .map(|n| match n {
            Node::Text(_) => n.clone(),
            Node::Element(e) => {
                let mut e = e.clone();
                if atomic(&e, vcx) {
                    e.style.used_mut().inh.ui.hidden = Some(true);
                    return Node::Element(e);
                }
                let list = parent.child(&e);
                let ink = if list.resolved() == group {
                    shadow.used_color(e.style.cascaded().current_color())
                } else {
                    transparent()
                };
                let s = e.style.used_mut();
                s.reset_text_box.text_shadow_ink = Some(ink);
                s.reset_background.background = None;
                s.reset_background.bg_image = None;
                s.reset_effects.shadows.clear();
                s.reset_border.outline = None;
                s.reset_border.border_colors = std::array::from_fn(|_| transparent().into());
                s.inh.text_ink.text_shadow = None;
                s.inh.text_ink.text_shadow_rest.clear();
                s.reset_text_box.text_shadow_raw = None;
                e.children = mask_nodes(&e.children, &list, group, shadow, vcx).into();
                Node::Element(e)
            }
        })
        .collect()
}

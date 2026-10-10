//! Маски цветовых групп для составных текстовых теней.

use super::{atomic, effective};
use crate::dom::Node;
use crate::render::{RenderOpts, paragraph};
use crate::style::computed::{Computed, Shadow};
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
    block: &[Shadow],
    nodes: &[Node],
    style: &Computed,
    opts: &RenderOpts,
) -> AnyElement {
    let on = block == group;
    let ink = if on { shadow.color } else { transparent() };
    let mut mask = style.clone();
    mask.color = Some(ink);
    mask.td_color = Some(ink);
    mask.emphasis_color = Some(ink);
    for decor in &mut mask.decors {
        decor.color = ink;
    }
    mask.text_shadow = None;
    mask.text_shadow_rest.clear();
    mask.text_shadow_raw = None;
    let masked = mask_nodes(nodes, block, &style.decors, &mask.decors, group, shadow);
    let copy = paragraph(&masked, &mask, opts);
    let copy = if shadow.blur > 0.5 {
        let mut g = crate::paint::effects::grouped_element::Grouped::new(copy);
        g.blur = shadow.blur * 0.5;
        g.into_any_element()
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
    parent: &[Shadow],
    parent_decors: &[crate::style::computed::Decor],
    parent_masked: &[crate::style::computed::Decor],
    group: &[Shadow],
    shadow: &Shadow,
) -> Vec<Node> {
    nodes
        .iter()
        .map(|n| match n {
            Node::Text(_) => n.clone(),
            Node::Element(e) => {
                let mut e = e.clone();
                if atomic(&e) {
                    e.style.hidden = Some(true);
                    return Node::Element(e);
                }
                let list = effective(&e, parent);
                let ink = if list.as_slice() == group {
                    shadow.color
                } else {
                    transparent()
                };
                let own_decors = e.style.decors.clone();
                let s = &mut e.style;
                s.color = Some(ink);
                s.td_color = Some(ink);
                s.emphasis_color = Some(ink);
                // Украшения предков красит тень предка: ведущие записи,
                // совпадающие с родительскими, берут маску родителя.
                for (i, decor) in s.decors.iter_mut().enumerate() {
                    decor.color = if i < parent_decors.len() && parent_decors[i] == own_decors[i] {
                        parent_masked.get(i).map_or(ink, |d| d.color)
                    } else {
                        ink
                    };
                }
                s.background = None;
                s.bg_image = None;
                s.shadows.clear();
                s.outline = None;
                s.border_color = Some(transparent());
                s.border_colors = [Some(transparent()); 4];
                s.text_shadow = None;
                s.text_shadow_rest.clear();
                s.text_shadow_raw = None;
                let masked_decors = e.style.decors.clone();
                let (own, masked) = if own_decors.is_empty() {
                    (parent_decors.to_vec(), parent_masked.to_vec())
                } else {
                    (own_decors, masked_decors)
                };
                e.children = mask_nodes(&e.children, &list, &own, &masked, group, shadow);
                Node::Element(e)
            }
        })
        .collect()
}

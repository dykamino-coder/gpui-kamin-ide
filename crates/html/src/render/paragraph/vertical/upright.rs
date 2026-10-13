//! Стопка стоячих глифов с продвижением в один кегль.

use crate::dom::Node;
use crate::render::paragraph::paragraph;
use crate::render::*;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};

/// CSS Writing Modes 4 §5.1: orientation belongs to each text run, including
/// an explicit mixed override inside an upright container. Atomic contents
/// belong to their own formatting context and cannot select this route.
pub(super) fn only_mixed_text(nodes: &[Node], inherited: &Computed) -> bool {
    fn visit(nodes: &[Node], style: &Computed, seen: &mut bool) -> bool {
        nodes.iter().all(|node| match node {
            Node::Text(text) if !text.trim().is_empty() => {
                *seen = true;
                style.upright != Some(true)
            }
            Node::Element(element)
                if crate::render::real_inline(element)
                    || element.style.display == Some(crate::style::computed::Display::Contents) =>
            {
                let child =
                    crate::style::cascade::inherit::inherit_unpainted(style, &element.style);
                visit(&element.children, &child, seen)
            }
            _ => true,
        })
    }
    let mut seen = false;
    visit(nodes, inherited, &mut seen) && seen
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn upright_paragraph(
    nodes: &[Node],
    inherited: &Computed,
    opts: &RenderOpts,
) -> AnyElement {
    if let Some(stack) = super::upright_central::upright_central(nodes, inherited, opts) {
        return stack;
    }
    let mut stack = inherited.clone();
    stack.vertical = None;
    stack.upright_stack = true;
    stack.break_word = Some(true);
    let em = match stack.font_size {
        Some(Len::Px(v)) => v,
        _ => opts.base_size(),
    };
    // Каждый стоячий глиф продвигает строку РОВНО на кегль (§7.4):
    // шаг стопки — кегль, а не своя высота строки; полоса переноса
    // уже одного глифа — в строку ложится ровно один знак (два узких
    // нуля вставали рядом, и стопка выходила короче).
    // Толщина вертикальной строки — LINE-HEIGHT, как у горизонтальной
    // (стопка глифов стоит в полосе высоты строки, повернутой набок):
    // читается ДО подмены шага стопки кеглем, иначе полоса всегда
    // равнялась кеглю (`vertical-alignment-vrl-022`).
    let lane = match stack.line_height {
        Some(Len::Px(v)) => v,
        Some(Len::Em(k)) | Some(Len::Pct(k)) => k * em,
        // До этой точки `ch` мог не разрешиться: стоячий ноль
        // продвигается на кегль (§7.4) — считаем сами.
        Some(Len::Ch(k)) => k * em,
        _ => em,
    };
    stack.line_height = Some(Len::Px(em));
    // Стопка набрана горизонтально, а физические лево и право строчной
    // коробки в вертикальном письме — БЛОЧНАЯ ось (css-writing-modes-4
    // §6.4, Abstract-Physical Mapping): её поля, отбивки и рамки строку не
    // удлиняют. В горизонтальной стопке они раздвигали знаки
    // (`line-box-height-vlr-011`: `border-left: 1.5em` у `<span>`
    // вставлял пустую строку между «2» и «3»).
    let nodes: Vec<Node> = nodes.iter().map(drop_block_axis_sides).collect();
    let inner = paragraph(&nodes, &stack, opts);
    div()
        .w(px(lane.max(em)))
        .flex_shrink_0()
        .flex()
        .justify_center()
        .child(div().w(px(em * 0.9)).flex_shrink_0().child(inner))
        .into_any_element()
}

/// Строчные элементы без физических левых и правых полей, отбивок и рамок.
fn drop_block_axis_sides(n: &Node) -> Node {
    match n {
        Node::Element(e) if e.inline && e.style.display.is_none() => {
            let mut e = e.clone();
            for sides in [
                &mut e.style.margin,
                &mut e.style.padding,
                &mut e.style.border_width,
            ] {
                sides.left = None;
                sides.right = None;
            }
            e.children = e.children.iter().map(drop_block_axis_sides).collect();
            Node::Element(e)
        }
        other => other.clone(),
    }
}

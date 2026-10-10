//! Стопка стоячих глифов с продвижением в один кегль.

use crate::dom::Node;
use crate::render::paragraph::paragraph;
use crate::render::*;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};

#[allow(clippy::too_many_arguments)]
pub(crate) fn upright_paragraph(
    nodes: &[Node],
    inherited: &Computed,
    opts: &RenderOpts,
) -> AnyElement {
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
    let inner = paragraph(nodes, &stack, opts);
    div()
        .w(px(lane.max(em)))
        .flex_shrink_0()
        .flex()
        .justify_center()
        .child(div().w(px(em * 0.9)).flex_shrink_0().child(inner))
        .into_any_element()
}

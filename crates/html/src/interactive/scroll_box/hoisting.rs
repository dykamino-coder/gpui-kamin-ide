//! Подъём потомков из scroll-коробки перед сборкой её содержимого.

use crate::dom::{Element, Node};
use crate::layout::options::RenderOpts;
use crate::layout::positioned::predicates::{edge_set, stays_positioned};
use crate::style::computed::{BoxStyle, Display};

/// Вынуть из поддерева ленты прокрутки абсолютных потомков, которым лента не
/// содержащий блок.
///
/// §11.1.1: предок обрезает ТОЛЬКО того потомка, для которого он содержащий
/// блок. У абсолютного элемента без позиционированного предка содержащий блок
/// — область просмотра (§10.1 п.4), и `overflow: scroll|auto` его не касается.
///
/// Лента строит поддерево в замыкании, которое зовёт
/// `ScrollArea::request_layout` — уже после `icb_close()`, и `icb_push` вернул
/// бы элемент назад «рисовать на месте». Поэтому кандидатов вынимаем ЗДЕСЬ.
///
/// Условия — те же, что у `to_icb`, плюс два ужесточения: только
/// НЕПОСРЕДСТВЕННЫЕ дети ленты и только при ОБЕИХ заданных осях (по свободной
/// оси место сообщает щуп, а он остался бы в замыкании).
pub(super) fn hoist_from_scroll(e: &mut Element, inherited: &BoxStyle, opts: &RenderOpts) {
    let vcx = opts.cx.vcx();
    use crate::style::computed::Position;
    if !opts.cx.positioning.icb_active() || opts.cx.paint_depth.inside() {
        return;
    }
    let merged = crate::style::cascade::materialize::box_style(inherited, &e.style, vcx);
    // Лента внутри позиционированного предка не выносит ничего: у её потомков
    // содержащий блок есть.
    if merged.cb_ancestor || crate::text::inline::establishes_cb(&merged) {
        return;
    }
    if matches!(
        merged.display,
        Some(Display::Flex)
            | Some(Display::InlineFlex)
            | Some(Display::Grid)
            | Some(Display::InlineGrid)
            | Some(Display::GridLanes)
    ) {
        return;
    }
    let take: Vec<bool> = (0..e.children.len())
        .map(|i| {
            let Node::Element(c) = &e.children[i] else {
                return false;
            };
            let x_set =
                edge_set(c.style.computed().inset.left) || edge_set(c.style.computed().inset.right);
            let y_set =
                edge_set(c.style.computed().inset.top) || edge_set(c.style.computed().inset.bottom);
            c.style.computed().position == Some(Position::Absolute)
                && c.style.computed().z_index.unwrap_or(0) >= 0
                && x_set
                && y_set
                && !stays_positioned(&e.children[i + 1..])
        })
        .collect();
    if !take.iter().any(|t| *t) {
        return;
    }
    let mut keep = Vec::with_capacity(e.children.len());
    for (i, child) in std::mem::take(&mut e.children).into_iter().enumerate() {
        if !take[i] {
            keep.push(child);
            continue;
        }
        // `blocks` на одном узле повторяет ВЕСЬ путь `to_icb`: строит элемент
        // и отдаёт открытому слою ICB. При обеих заданных осях щуп не нужен,
        // поэтому список возвращается пустым.
        let left = opts
            .cx
            .build
            .blocks(std::slice::from_ref(&child), &merged, opts);
        if left.is_empty() {
            continue;
        }
        // Условия предиката разошлись с `to_icb`: узел остаётся на месте, а не
        // теряется.
        drop(left);
        keep.push(child);
    }
    e.children = keep;
}

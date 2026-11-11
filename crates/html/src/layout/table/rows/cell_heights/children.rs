//! Percentage heights of a cell's direct children, resolved before the row is
//! measured. The children are edited only when one has such a share: an edit
//! parts the cell from the document's child list and its intrinsic sizes.

use crate::dom::{Element, Node};
use crate::style::values::value::Len;

/// A cell with no height of its own: the share of a clipping child is taken
/// from the child's own `min-height`.
pub(super) fn anchor_child_shares(cell: &mut Element) {
    if !cell.children.iter().any(|n| anchored_share(n).is_some()) {
        return;
    }
    for child in cell.children.iter_mut() {
        if let Some(h) = anchored_share(child)
            && let Node::Element(el) = child
        {
            el.style.used_mut().reset_box_layout.height = Some(Len::Px(h));
        }
    }
}
/// The height of a clipping child with a percentage height, from its own
/// `min-height` (the anchor of a cell with no height of its own).
fn anchored_share(child: &Node) -> Option<f32> {
    let Node::Element(el) = child else {
        return None;
    };
    let style = el.style.used();
    let layout = &style.reset_box_layout;
    match (layout.height, layout.min_height) {
        (Some(Len::Pct(k)), Some(Len::Px(m)))
            if layout
                .overflow_y
                .is_some_and(|o| o != crate::style::computed::Overflow::Visible) =>
        {
            Some(m * k)
        }
        _ => None,
    }
}

/// Доли прямых детей ячейки с заданной высотой `h`.
pub(super) fn resolve_child_percents(cell: &mut Element, h: f32) {
    let share = |l| matches!(l, Some(Len::Pct(_)));
    if !cell.children.iter().any(|n| {
        matches!(n, Node::Element(el) if {
            let style = el.style.used();
            let layout = &style.reset_box_layout;
            share(layout.height) || share(layout.max_height) || share(layout.min_height)
        })
    }) {
        return;
    }
    // Процентная высота ПРЯМОГО ребёнка заранее решается от ЗАДАННОЙ
    // высоты ячейки только у замещаемого и у прокручиваемого ребёнка
    // (Blink `length_utils.cc`: replaced descendants of a fixed-height
    // cell; css-tables-3 §row-layout: scrollable percentage children of a
    // restricted cell). Остальные доли в замере ряда — auto («percentages
    // being considered 0px», css-tables-3 §computing-the-table-height) и
    // решаются от итоговой высоты ячейки при раскладке: заранее решённые
    // 200% растили ряд вдвое (`table-cell-expansion-002`).
    for child in cell.children.iter_mut() {
        if let Node::Element(el) = child
            && let Some(Len::Pct(k)) = el.style.used().reset_box_layout.height
            && (scrollable(el) || crate::layout::box_kind::replaced_tag(el))
        {
            el.style.used_mut().reset_box_layout.height = Some(Len::Px(h * k));
        }
        // Пороги той же долей — от той же заданной высоты ячейки
        // (CSS 2.1 §10.7: доля `max-height`/`min-height` считается
        // как у `height`). Нерешённая доля у нас отбрасывается, и
        // заменяемый ребёнок шёл природным размером: `<canvas
        // 200×200 max-height: 100%>` в ячейке высотой 100 давал
        // 200×200 вместо 100×100
        // (`percent-height-replaced-in-percent-cell-002`).
        if let Node::Element(el) = child
            && let Some(Len::Pct(k)) = el.style.used().reset_box_layout.max_height
        {
            el.style.used_mut().reset_box_layout.max_height = Some(Len::Px(h * k));
        }
        if let Node::Element(el) = child
            && let Some(Len::Pct(k)) = el.style.used().reset_box_layout.min_height
        {
            el.style.used_mut().reset_box_layout.min_height = Some(Len::Px(h * k));
        }
    }
}

/// Прокручиваемый ребёнок: `overflow-y: auto | scroll` (Blink
/// `SetIsRestrictedBlockSizeTableCellChild`; `hidden`/`clip` не в счёт).
fn scrollable(el: &Element) -> bool {
    el.style.used().reset_box_layout.overflow_y == Some(crate::style::computed::Overflow::Scroll)
}

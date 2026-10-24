//! Стопка, всю геометрию которой даёт раскладка копий до балансировки.

use crate::document::state::ValueCx;
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::block::margins::fragment_box;
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::Shape;
use crate::layout::fragment::flow::{empty_floats, independent_copy};
use crate::layout::fragment::line_shape::resolved_lengths;
use crate::style::computed::BoxStyle;
use std::cell::RefCell;

/// Все соседи должны измеряться до плана: предварительные планы clone и
/// вложенных рядов иначе прочитали бы ещё не разложенные высоты соседей.
pub(crate) fn children(
    host: &Element,
    merged: &BoxStyle,
    vcx: ValueCx<'_>,
    width: f32,
    measured: &RefCell<Vec<(u64, f32)>>,
) -> Option<Vec<(Element, Shape)>> {
    // ADR P-19 TRANSITIONAL(F2-1): clearance следующей копии зависит от float bands предыдущей.
    if crate::layout::fragment::flow::copy_clearance(&host.children)
        || merged.text_box_trim_start
        || merged.text_box_trim_end
    {
        return None;
    }
    let flow: Vec<_> = host.children.iter().filter(|n| !is_blank(n)).collect();
    if !flow.iter().all(|n| match n {
        // ADR P-19 TRANSITIONAL(F2-2): static position зависит от разложенных соседей;
        // удаление OOF до замера теряет его исходное место в потоке.
        Node::Element(c) => {
            !out_of_flow(&c.style.used()) && (independent_copy(c) || empty_floats(c))
        }
        _ => false,
    }) {
        return None;
    }
    let kids: Option<Vec<_>> = flow
        .into_iter()
        .filter_map(|n| match n {
            Node::Element(c) if !out_of_flow(&c.style.used()) => Some(c),
            _ => None,
        })
        .map(|c| {
            let c = resolved_lengths(c, merged, vcx);
            let c = fragment_box(c, width, vcx);
            let (margin_top, margin_bottom) = crate::layout::block::margins::fragment_margins(&c)?;
            Some((
                c,
                Shape {
                    margin_top,
                    margin_bottom,
                    ..Shape::default()
                },
            ))
        })
        .collect();
    let kids = kids?;
    measured
        .borrow_mut()
        .extend(kids.iter().map(|(c, _)| (c.node_id, width)));
    Some(kids)
}

/// До определения ширины сохраняется исходное дерево для intrinsic sizing.
/// Числовые поля этой записи не участвуют в плане: deferred строит её заново.
pub(crate) fn pending(
    host: &Element,
    merged: &BoxStyle,
    vcx: ValueCx<'_>,
) -> Option<Vec<(Element, Shape)>> {
    // ADR P-19 TRANSITIONAL(F2-1): clearance следующей копии зависит от float bands предыдущей.
    if crate::layout::fragment::flow::copy_clearance(&host.children)
        || merged.text_box_trim_start
        || merged.text_box_trim_end
    {
        return None;
    }
    host.children
        .iter()
        .filter(|n| !is_blank(n))
        .map(|n| {
            let Node::Element(c) = n else { return None };
            if !independent_copy(c) && !empty_floats(c) {
                return None;
            }
            let c = resolved_lengths(c, merged, vcx);
            Some((c, Shape::default()))
        })
        .collect()
}

/// Вертикальный поток: физическое дерево остаётся у раскладки, логические
/// свойства краёв читает только метадата рекордера, без оценки размера.
pub(crate) fn vertical_children(
    host: &Element,
    merged: &BoxStyle,
    vcx: ValueCx<'_>,
    width: f32,
    reverse: bool,
    measured: &RefCell<Vec<(u64, f32)>>,
) -> Option<Vec<(Element, Shape)>> {
    // ADR P-19 TRANSITIONAL(F2-1): clearance следующей копии зависит от float bands предыдущей.
    if crate::layout::fragment::flow::copy_clearance(&host.children)
        || merged.text_box_trim_start
        || merged.text_box_trim_end
    {
        return None;
    }
    let kids: Option<Vec<_>> = host
        .children
        .iter()
        .filter(|n| !is_blank(n))
        .map(|n| {
            let Node::Element(c) = n else { return None };
            if !crate::layout::fragment::flow::vertical_copy(c, reverse) {
                return None;
            }
            let c = resolved_lengths(c, merged, vcx);
            let c = crate::layout::block::margins::vertical_fragment_box(c, width, reverse, vcx);
            let margin = |l| match l {
                None | Some(crate::style::values::value::Len::Auto) => Some(0.0),
                Some(crate::style::values::value::Len::Px(v)) => Some(v),
                _ => None,
            };
            let (start, end) = if reverse {
                (c.style.used().margin.right, c.style.used().margin.left)
            } else {
                (c.style.used().margin.left, c.style.used().margin.right)
            };
            Some((
                c,
                Shape {
                    margin_top: margin(start)?,
                    margin_bottom: margin(end)?,
                    ..Shape::default()
                },
            ))
        })
        .collect();
    let kids = kids?;
    measured
        .borrow_mut()
        .extend(kids.iter().map(|(c, _)| (c.node_id, width)));
    Some(kids)
}

/// Вертикальное исходное дерево до определения строчного размера.
pub(crate) fn pending_vertical(
    host: &Element,
    merged: &BoxStyle,
    vcx: ValueCx<'_>,
    reverse: bool,
) -> Option<Vec<(Element, Shape)>> {
    // ADR P-19 TRANSITIONAL(F2-1): clearance следующей копии зависит от float bands предыдущей.
    if crate::layout::fragment::flow::copy_clearance(&host.children)
        || merged.text_box_trim_start
        || merged.text_box_trim_end
    {
        return None;
    }
    host.children
        .iter()
        .filter(|n| !is_blank(n))
        .map(|n| {
            let Node::Element(c) = n else { return None };
            if !crate::layout::fragment::flow::vertical_copy(c, reverse) {
                return None;
            }
            let c = resolved_lengths(c, merged, vcx);
            Some((c, Shape::default()))
        })
        .collect()
}

#[cfg(test)]
mod tests;

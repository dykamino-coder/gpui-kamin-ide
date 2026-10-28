//! Стопка, всю геометрию которой даёт раскладка копий до балансировки.

use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::block::margins::fragment_box;
use crate::layout::box_kind::out_of_flow;
use crate::layout::cx::FrameCx;
use crate::layout::fragment::FlowMargins;
use crate::layout::fragment::flow::{empty_floats, independent_copy};
use crate::layout::fragment::line_shape::resolved_lengths;
use crate::style::computed::BoxStyle;
use std::cell::RefCell;

/// Все соседи должны измеряться до плана: предварительные планы clone и
/// вложенных рядов иначе прочитали бы ещё не разложенные высоты соседей.
pub(crate) fn children(
    host: &Element,
    merged: &BoxStyle,
    vcx: FrameCx<'_>,
    width: f32,
    measured: &RefCell<Vec<(u64, f32)>>,
) -> Option<Vec<(Element, FlowMargins)>> {
    if merged.text_box_trim_start || merged.text_box_trim_end {
        return None;
    }
    let nodes = crate::layout::fragment::flow::copy_flow(&host.children);
    let flow: Vec<_> = nodes.iter().filter(|n| !is_blank(n)).collect();
    if !flow.iter().all(|n| match n {
        // TRANSITIONAL(F2-2): Фильтрация OOF перед children теряет static position среди соседей; удалить отказ в P08 после записи static rect и отдельного OOF resume.
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
            let c = fragment_box(c, width, vcx.values);
            let (margin_top, margin_bottom) = crate::layout::block::margins::fragment_margins(&c)?;
            Some((
                c,
                FlowMargins {
                    margin_top,
                    margin_bottom,
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

/// Смешанные поточные дети до определения ширины: запись на каждого ребёнка,
/// чью меру и разрывы при известной ширине даст раскладка его копии
/// (`plan_at`); внепоточные остаются соседями стопки.
pub(crate) fn pending_mixed(
    host: &Element,
    merged: &BoxStyle,
    vcx: FrameCx<'_>,
) -> Option<Vec<(Element, FlowMargins)>> {
    host.children
        .iter()
        .filter(|n| !is_blank(n))
        .filter(|n| !matches!(n, Node::Element(c) if out_of_flow(&c.style.used())))
        .map(|n| {
            let Node::Element(c) = n else { return None };
            // Те же дети, что меряет `measure_children` при известной ширине.
            let used = c.style.used();
            let block = !c.inline
                && matches!(
                    used.position,
                    None | Some(crate::style::computed::Position::Relative)
                )
                && (used.float.unwrap_or(0) == 0 || crate::layout::float::block_like_float(&used));
            block.then(|| (resolved_lengths(c, merged, vcx), FlowMargins::default()))
        })
        .collect()
}

/// До определения ширины сохраняется исходное дерево для intrinsic sizing.
/// Числовые поля этой записи не участвуют в плане: deferred строит её заново.
pub(crate) fn pending(
    host: &Element,
    merged: &BoxStyle,
    vcx: FrameCx<'_>,
) -> Option<Vec<(Element, FlowMargins)>> {
    if merged.text_box_trim_start || merged.text_box_trim_end {
        return None;
    }
    let nodes = crate::layout::fragment::flow::copy_flow(&host.children);
    nodes
        .iter()
        .filter(|n| !is_blank(n))
        .map(|n| {
            let Node::Element(c) = n else { return None };
            if !independent_copy(c) && !empty_floats(c) {
                return None;
            }
            let c = resolved_lengths(c, merged, vcx);
            Some((c, FlowMargins::default()))
        })
        .collect()
}

/// Вертикальный поток: физическое дерево остаётся у раскладки, логические
/// свойства краёв читает только метадата рекордера, без оценки размера.
pub(crate) fn vertical_children(
    host: &Element,
    merged: &BoxStyle,
    vcx: FrameCx<'_>,
    width: f32,
    reverse: bool,
    measured: &RefCell<Vec<(u64, f32)>>,
) -> Option<Vec<(Element, FlowMargins)>> {
    if merged.text_box_trim_start || merged.text_box_trim_end {
        return None;
    }
    let nodes = crate::layout::fragment::flow::copy_flow(&host.children);
    if !nodes.iter().filter(|n| !is_blank(n)).all(|n| {
        matches!(n, Node::Element(c) if crate::layout::fragment::flow::vertical_copy(c, reverse))
    }) {
        return None;
    }
    nodes
        .iter()
        .filter(|n| !is_blank(n))
        .map(|n| {
            let Node::Element(c) = n else { return None };
            vertical_kid(
                resolved_lengths(c, merged, vcx),
                vcx,
                width,
                reverse,
                measured,
            )
        })
        .collect()
}

/// Ребёнок вертикальной стопки, чью высоту и разрывы даёт раскладка его копии;
/// поля — по блочной оси письма колонок.
pub(crate) fn vertical_kid(
    c: Element,
    vcx: FrameCx<'_>,
    width: f32,
    reverse: bool,
    measured: &RefCell<Vec<(u64, f32)>>,
) -> Option<(Element, FlowMargins)> {
    let c = crate::layout::block::margins::vertical_fragment_box(c, width, reverse, vcx.values);
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
    let shape = FlowMargins {
        margin_top: margin(start)?,
        margin_bottom: margin(end)?,
    };
    measured.borrow_mut().push((c.node_id, width));
    Some((c, shape))
}

/// Вертикальное исходное дерево до определения строчного размера.
pub(crate) fn pending_vertical(
    host: &Element,
    merged: &BoxStyle,
    vcx: FrameCx<'_>,
    reverse: bool,
) -> Option<Vec<(Element, FlowMargins)>> {
    if merged.text_box_trim_start || merged.text_box_trim_end {
        return None;
    }
    let nodes = crate::layout::fragment::flow::copy_flow(&host.children);
    nodes
        .iter()
        .filter(|n| !is_blank(n))
        .map(|n| {
            let Node::Element(c) = n else { return None };
            if !crate::layout::fragment::flow::vertical_copy(c, reverse) {
                return None;
            }
            let c = resolved_lengths(c, merged, vcx);
            Some((c, FlowMargins::default()))
        })
        .collect()
}

#[cfg(test)]
mod tests;

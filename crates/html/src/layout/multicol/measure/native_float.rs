//! Отложенная ширина не требует оценки: floats меряет тот же решатель продолжения.

use crate::document::state::ValueCx;
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::block::margins::{fragment_box, fragment_margins};
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::flow::{empty_floats, independent_copy};
use crate::layout::fragment::line_shape::resolved_lengths;
use crate::layout::fragment::{Shape, paginate};
use crate::style::computed::{BoxStyle, Display};
use std::cell::RefCell;

pub(crate) fn float_copy(e: &Element, vcx: ValueCx<'_>) -> bool {
    fn separate(e: &Element) -> bool {
        e.style.used().bdb_clone
            || e.style.used().text_box_trim_start
            || e.style.used().text_box_trim_end
            || e.marks.host_trim_start
            || e.marks.host_trim_end
            || e.style.used().wm().vertical()
            || e.style.used().column_span == Some(true)
            || crate::layout::multicol::spanner::multicol_container(&e.style.used())
            || matches!(e.style.used().display, Some(Display::Flex | Display::Grid))
            || (out_of_flow(&e.style.used())
                && (e.style.used().float.unwrap_or(0) == 0
                    || !crate::layout::fragment::clone::content_monolith_blockified(e)))
            || e.children
                .iter()
                .any(|n| matches!(n, Node::Element(k) if separate(k)))
    }
    !separate(e)
        && paginate::gate(e, None, false, vcx)
        && paginate::candidates(e, vcx).is_some_and(|v| {
            v.iter()
                .any(|(_, c)| matches!(c, paginate::Cand::Float { .. }))
        })
}

pub(crate) fn pending(
    host: &Element,
    merged: &BoxStyle,
    vcx: ValueCx<'_>,
) -> Option<Vec<(Element, Shape)>> {
    prepare(host, merged, vcx, None, &RefCell::default())
}

pub(crate) fn children(
    host: &Element,
    merged: &BoxStyle,
    vcx: ValueCx<'_>,
    width: f32,
    measured: &RefCell<Vec<(u64, f32)>>,
) -> Option<Vec<(Element, Shape)>> {
    prepare(host, merged, vcx, Some(width), measured)
}

fn prepare(
    host: &Element,
    merged: &BoxStyle,
    vcx: ValueCx<'_>,
    width: Option<f32>,
    measured: &RefCell<Vec<(u64, f32)>>,
) -> Option<Vec<(Element, Shape)>> {
    if merged.text_box_trim_start || merged.text_box_trim_end {
        return None;
    }
    let nodes = crate::layout::fragment::flow::copy_flow(&host.children);
    let kids: Option<Vec<_>> = nodes
        .iter()
        .filter(|n| !is_blank(n))
        .map(|n| {
            let Node::Element(c) = n else { return None };
            let independent = independent_copy(c) || empty_floats(c);
            if !independent && !float_copy(c, vcx) {
                return None;
            }
            let mut c = resolved_lengths(c, merged, vcx);
            let Some(w) = width else {
                // Поля разрешаются по настоящей ширине в children(), как у
                // остальных native-копий; pending не нуждается в их оценке.
                return Some((c, Shape::default()));
            };
            c = fragment_box(c, w, vcx);
            if !independent {
                c.marks.paginate = true;
            }
            let (margin_top, margin_bottom) = fragment_margins(&c)?;
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
    if let Some(width) = width {
        measured
            .borrow_mut()
            .extend(kids.iter().map(|(c, _)| (c.node_id, width)));
    }
    Some(kids)
}

#[cfg(test)]
mod tests;

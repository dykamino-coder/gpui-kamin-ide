//! Поиск коробки роста и рост внутри многострочного flex-ряда теми же смещениями строк, что у
//! меры (`flex_lines::row_geometry`): точка на границе строки двигает ВСЮ строку полями её
//! элементов (разрыв класса A, css-flexbox-1 §12), точка внутри элемента спускается в него,
//! точка в уже усечённом зазоре строки рост не даёт (css-gaps §gaps).

use super::pushed_box_at;
use crate::document::state::ValueCx;
use crate::dom::{Element, Node};
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::flex_lines::row_geometry::{collect_rows, row_container};
use crate::layout::fragment::flex_lines::row_main_size;
use crate::style::values::value::Len;
use crate::style::values::value::px_auto_zero;

pub(super) fn pushed_row_at(
    c: &Element,
    at: f32,
    depth: u8,
    forced: bool,
    vcx: ValueCx<'_>,
) -> Option<Option<u64>> {
    if !row_container(c) {
        return None;
    }
    let rows = collect_rows(c, row_main_size(c, vcx)?, depth, ShapeCx::COLUMNS, vcx)?;
    let top =
        px_auto_zero(c.style.used().padding.top)? + px_auto_zero(c.style.used().borders().top)?;
    let gap = px_auto_zero(c.style.used().gap.and_then(|g| g.0))?.max(0.0);
    let mut y = top;
    for (ri, row) in rows.iter().enumerate() {
        if ri > 0 {
            if gap_cut(at, y, gap) {
                return Some(None);
            }
            if (at - y).abs() < 0.01 || (at - y - gap).abs() < 0.01 {
                return Some(row.items.first().map(|i| i.element.node_id));
            }
            y += gap;
        }
        for item in &row.items {
            let start = y + item.shape.margin_top;
            if at > start - 0.01
                && at < start + item.shape.height - 0.01
                && let Some(id) = pushed_box_at(&item.element, at - start, depth - 1, forced, vcx)
            {
                return Some(Some(id));
            }
        }
        y += row.height;
    }
    Some(None)
}

pub(crate) fn grow_row_at(c: &mut Element, at: f32, grow: f32, vcx: ValueCx<'_>) -> bool {
    let lines = crate::layout::fragment::LineCx::child(c, vcx);
    let vcx = vcx.with_lines(lines.as_ref().or(vcx.lines));
    let Some(rows) =
        row_main_size(c, vcx).and_then(|main| collect_rows(c, main, 4, ShapeCx::COLUMNS, vcx))
    else {
        return false;
    };
    let top = px_auto_zero(c.style.used().padding.top).unwrap_or(0.0)
        + px_auto_zero(c.style.used().borders().top).unwrap_or(0.0);
    let gap = px_auto_zero(c.style.used().gap.and_then(|g| g.0))
        .unwrap_or(0.0)
        .max(0.0);
    let mut y = top;
    for (ri, row) in rows.iter().enumerate() {
        if ri > 0 && gap_cut(at, y, gap) {
            return false;
        }
        if ri > 0 && ((at - y).abs() < 0.01 || (at - y - gap).abs() < 0.01) {
            // A class-A break moves the whole row; a margin on just the first
            // item would instead move that item within its parallel flow (12).
            for item in &row.items {
                let Some(Node::Element(k)) = c.children.get_mut(item.source) else {
                    return false;
                };
                k.style.used_mut().margin.top = Some(Len::Px(
                    px_auto_zero(k.style.used().margin.top).unwrap_or(0.0) + grow,
                ));
            }
            if let Some(Len::Px(h)) = c.style.used().height {
                c.style.used_mut().height = Some(Len::Px(h + grow));
            }
            return true;
        }
        if ri > 0 {
            y += gap;
        }
        y += row.height;
    }
    false
}

// A cut whose continuation skips a gutter already suppresses that gutter.
// Expanding the row here would restore the discarded space (css-gaps §gaps).
fn gap_cut(at: f32, start: f32, gap: f32) -> bool {
    gap > 0.0 && at >= start - 0.01 && at <= start + gap + 0.01
}

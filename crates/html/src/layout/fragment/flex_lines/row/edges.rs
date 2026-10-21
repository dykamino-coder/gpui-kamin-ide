//! Разрыв на краю многострочного flex-ряда (css-flexbox-1 §12): `break-before`/`avoid`
//! любого элемента первой строки и `break-after`/`avoid` любого элемента последней
//! переходят на край контейнера; строки собираются той же мерой (`collect_lines`).

use super::line_layout::collect_lines;
use super::measure::main_size;
use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::layout::fragment::breaks::{edge_avoid, edge_break};

pub(crate) fn row_break_edge(
    c: &Element,
    last: bool,
    avoid: bool,
    vcx: ValueCx<'_>,
) -> Option<bool> {
    let lines = crate::layout::fragment::LineCx::child(c, vcx);
    let vcx = vcx.with_lines(lines.as_ref().or(vcx.lines));
    let lines = collect_lines(c, main_size(c, vcx)?, 4)?;
    let edge = if last { lines.last() } else { lines.first() }?;
    Some(edge.iter().any(|(_, k)| {
        if avoid {
            edge_avoid(k, last, vcx)
        } else {
            edge_break(k, last, vcx)
        }
    }))
}

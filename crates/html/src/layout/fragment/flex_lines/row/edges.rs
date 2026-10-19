//! CSS Flexbox section 12 propagates every item's break value on the edge line.

use super::line_layout::collect_lines;
use super::measure::main_size;
use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::layout::fragment::LineScope;
use crate::layout::fragment::breaks::{edge_avoid, edge_break};

pub(crate) fn row_break_edge(
    c: &Element,
    last: bool,
    avoid: bool,
    vcx: ValueCx<'_>,
) -> Option<bool> {
    let _scope = LineScope::enter(c, vcx);
    let lines = collect_lines(c, main_size(c)?, 4)?;
    let edge = if last { lines.last() } else { lines.first() }?;
    Some(edge.iter().any(|(_, k)| {
        if avoid {
            edge_avoid(k, last, vcx)
        } else {
            edge_break(k, last, vcx)
        }
    }))
}

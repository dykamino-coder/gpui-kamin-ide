//! A table with one non-spanning cell per row has no parallel cell flow.
//!
//! CSS Break 3 §2.1 distinguishes parallel flows by the actual cells, not by
//! the table's display role. Anonymous table fixup must precede this decision.

use super::table_box;
use crate::dom::{Element, Node};
use crate::layout::table::anon::fixup_table_children;
use crate::render::is_blank;
use crate::style::computed::Display;

pub(crate) fn serial_table_parallel(
    table: &Element,
    depth: u8,
    parallel: impl Fn(&Element, u8) -> bool,
) -> Option<bool> {
    if depth == 0 || !table_box(table) {
        return None;
    }
    let fixed = fixup_table_children(&table.children);
    let mut rows = Vec::new();
    let mut nested = false;
    for n in fixed.iter().filter(|n| !is_blank(n)) {
        let Node::Element(part) = n else { return None };
        if part.tag == "caption" || part.style.is_caption == Some(true) {
            nested |= parallel(part, depth - 1);
        } else if part.tag == "tr" || part.style.display == Some(Display::TableRow) {
            rows.push(part);
        } else if matches!(part.tag.as_str(), "thead" | "tbody" | "tfoot")
            || part.style.row_group_kind.is_some()
        {
            for n in part.children.iter().filter(|n| !is_blank(n)) {
                let Node::Element(row) = n else { return None };
                if row.tag != "tr" && row.style.display != Some(Display::TableRow) {
                    return None;
                }
                rows.push(row);
            }
        } else {
            return None;
        }
    }
    if rows.is_empty() {
        return None;
    }
    for row in rows {
        let mut cells = row.children.iter().filter(|n| !is_blank(n));
        let Some(Node::Element(cell)) = cells.next() else {
            return None;
        };
        if cells.next().is_some() || !crate::layout::table::is_cell(cell) {
            return None;
        }
        // A spanning cell may overlap another row's cell even when each DOM
        // row has only one originating cell. Keep that parallel case intact.
        if ["rowspan", "colspan"].iter().any(|name| {
            cell.attr(name)
                .is_some_and(|v| v.parse::<usize>().ok() != Some(1))
        }) {
            return None;
        }
        nested |= cell
            .children
            .iter()
            .any(|n| matches!(n, Node::Element(child) if parallel(child, depth - 1)));
    }
    Some(nested)
}

#[cfg(test)]
mod tests;

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
mod tests {
    //! Check anonymous serial cell topology and preserve intrinsic monolithic cuts.

    use super::*;
    use crate::layout::fragment::ShapeCx;
    use crate::layout::table::anon::anon_element;
    use crate::style::values::value::Len;

    fn block() -> Node {
        let mut block = anon_element("div", vec![]);
        block.style.height = Some(Len::Px(100.0));
        block.style.contain_size = Some(true);
        Node::Element(block)
    }

    fn table(children: Vec<Node>) -> Element {
        let mut table = anon_element("div", children);
        table.style.display = Some(Display::Table);
        table
    }

    #[test]
    fn anonymous_cell_flow_is_serial_but_multiple_cells_are_parallel() {
        let serial = table(vec![block(), block()]);
        assert_eq!(serial_table_parallel(&serial, 4, |_, _| false), Some(false));
        let cells = vec![
            Node::Element(anon_element("td", vec![block()])),
            Node::Element(anon_element("td", vec![block()])),
        ];
        let parallel = table(vec![Node::Element(anon_element("tr", cells))]);
        assert_eq!(serial_table_parallel(&parallel, 4, |_, _| false), None);
    }

    #[test]
    fn rowspans_can_overlap_cells_from_later_rows() {
        let mut spanning = anon_element("td", vec![block()]);
        spanning.attrs.push(("rowspan".into(), "2".into()));
        let table = table(vec![
            Node::Element(anon_element("tr", vec![Node::Element(spanning)])),
            Node::Element(anon_element(
                "tr",
                vec![Node::Element(anon_element("td", vec![block()]))],
            )),
        ]);
        assert_eq!(serial_table_parallel(&table, 4, |_, _| false), None);
    }

    #[test]
    fn parallel_descendants_of_a_serial_cell_still_block_shared_overflow() {
        let mut grid = anon_element("div", vec![]);
        grid.style.display = Some(Display::Grid);
        let table = table(vec![Node::Element(grid)]);
        assert_eq!(
            serial_table_parallel(&table, 4, |e, _| e.style.display == Some(Display::Grid)),
            Some(true)
        );
    }

    #[test]
    fn satisfied_height_floor_retains_intrinsic_cell_breaks() {
        let mut table = table(vec![block(), block()]);
        table.style.height = Some(Len::Px(200.0));
        let shape = super::super::table_shape(&table, 4, ShapeCx::COLUMNS).unwrap();
        assert_eq!(shape.0, 200.0);
        assert!(shape.3.iter().any(|&(need, _)| (need - 100.0).abs() < 0.01));
        assert!(
            shape
                .5
                .iter()
                .any(|&(start, end)| start == 0.0 && end == 100.0)
        );
        table.style.height = Some(Len::Px(300.0));
        assert!(super::super::table_shape(&table, 4, ShapeCx::COLUMNS).is_none());
    }
}

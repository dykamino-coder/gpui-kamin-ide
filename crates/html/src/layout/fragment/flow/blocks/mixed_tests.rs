//! Разрывы сетки в ячейке таблицы читают дорожки вложенной раскладки.

use super::{BlockBox, Laid, Pushed, RowPush, TableFrame};
use crate::dom::{Element, Node};
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::flow::{FlowGeometry, independent_copy};

fn authored(nodes: &[Node]) -> &Element {
    fn find(nodes: &[Node]) -> Option<&Element> {
        nodes.iter().find_map(|n| match n {
            Node::Element(e) if e.tag == "div" => Some(e),
            Node::Element(e) => find(&e.children),
            _ => None,
        })
    }
    find(nodes).expect("authored div")
}

#[test]
fn table_cell_grid_uses_laid_tracks_for_breaks_and_growth() {
    let doc = crate::dom::parse(
        "<div style='display:table'><div style='display:table-row'><div style='display:table-cell'><div style='display:grid'><div style='height:10px'></div><div style='height:10px;break-before:column'></div></div></div></div></div>",
        "",
    );
    let root = authored(&doc);
    assert!(independent_copy(root));
    let b = BlockBox::collect(root, ShapeCx::COLUMNS, doc.values());
    let mut grids = Vec::new();
    b.grids(&mut grids);
    assert_eq!(grids.len(), 1);
    let (grid, items) = &grids[0];
    assert_eq!(items.len(), 2);
    let mut laid = Laid::of_boxes(&[
        (root.node_id, 0.0, 137.0),
        (*grid, 0.0, 137.0),
        (items[0], 0.0, 61.0),
        (items[1], 61.0, 137.0),
    ]);
    laid.grids.insert(*grid, vec![(0.0, 61.0), (61.0, 137.0)]);
    laid.tables.insert(
        b.table.as_ref().expect("table").key,
        TableFrame {
            table: (0.0, 137.0),
            grid: (0.0, 137.0),
            rows: vec![(0.0, 137.0)],
        },
    );
    let breaks = b
        .breaks(137.0, &laid)
        .finish(FlowGeometry::default(), 137.0);
    // Между соприкасающимися дорожками нет зазора для усечения;
    // break-before элемента передаётся на границу ряда как forced.
    assert_eq!(breaks.forced, vec![61.0]);
    assert_eq!(
        b.pushed_at(61.0, &laid),
        Some(Pushed::Row(RowPush {
            grid: *grid,
            track: 0,
            items: vec![items[0]],
        }))
    );
    let mut descendants = std::collections::HashMap::new();
    b.descendants(&mut descendants);
    assert_eq!(descendants[grid], *items);
}

#[test]
fn block_containing_table_and_grid_has_one_measured_tree() {
    let doc = crate::dom::parse(
        "<div><div style='display:table'><div style='display:table-row'><div style='display:table-cell'>table text</div></div></div><div style='display:grid'><div style='height:10px'></div></div></div>",
        "",
    );
    assert!(independent_copy(authored(&doc)));
}

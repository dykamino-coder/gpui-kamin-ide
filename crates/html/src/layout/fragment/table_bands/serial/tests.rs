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
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let serial = table(vec![block(), block()]);
    assert_eq!(
        serial_table_parallel(&serial, 4, |_, _| false, vcx),
        Some(false)
    );
    let cells = vec![
        Node::Element(anon_element("td", vec![block()])),
        Node::Element(anon_element("td", vec![block()])),
    ];
    let parallel = table(vec![Node::Element(anon_element("tr", cells))]);
    assert_eq!(serial_table_parallel(&parallel, 4, |_, _| false, vcx), None);
}

#[test]
fn rowspans_can_overlap_cells_from_later_rows() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut spanning = anon_element("td", vec![block()]);
    spanning.attrs.push(("rowspan".into(), "2".into()));
    let table = table(vec![
        Node::Element(anon_element("tr", vec![Node::Element(spanning)])),
        Node::Element(anon_element(
            "tr",
            vec![Node::Element(anon_element("td", vec![block()]))],
        )),
    ]);
    assert_eq!(serial_table_parallel(&table, 4, |_, _| false, vcx), None);
}

#[test]
fn parallel_descendants_of_a_serial_cell_still_block_shared_overflow() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut grid = anon_element("div", vec![]);
    grid.style.display = Some(Display::Grid);
    let table = table(vec![Node::Element(grid)]);
    assert_eq!(
        serial_table_parallel(
            &table,
            4,
            |e, _| e.style.display == Some(Display::Grid),
            vcx
        ),
        Some(true)
    );
}

#[test]
fn satisfied_height_floor_retains_intrinsic_cell_breaks() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut table = table(vec![block(), block()]);
    table.style.height = Some(Len::Px(200.0));
    let shape = super::super::table_shape(&table, 4, ShapeCx::COLUMNS, vcx).unwrap();
    assert_eq!(shape.0, 200.0);
    assert!(shape.3.iter().any(|&(need, _)| (need - 100.0).abs() < 0.01));
    assert!(
        shape
            .5
            .iter()
            .any(|&(start, end)| start == 0.0 && end == 100.0)
    );
    table.style.height = Some(Len::Px(300.0));
    assert!(super::super::table_shape(&table, 4, ShapeCx::COLUMNS, vcx).is_none());
}

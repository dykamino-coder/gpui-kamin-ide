//! Regression checks for originating-row growth and row-group span limits.

use super::row_geometry;
use crate::dom::{Element, Node};
use crate::layout::fragment::ShapeCx;
use crate::layout::table::anon::anon_element;
use crate::style::values::value::Len;

fn block(height: f32) -> Node {
    let mut e = anon_element("div", Vec::new());
    e.style.height = Some(Len::Px(height));
    Node::Element(e)
}

fn cell(height: f32, span: &str) -> Node {
    let mut e = anon_element("td", vec![block(height)]);
    e.attrs.push(("rowspan".into(), span.into()));
    Node::Element(e)
}

fn row(children: Vec<Node>) -> Element {
    anon_element("tr", children)
}

#[test]
fn clipped_span_uses_the_remaining_rows_in_its_group() {
    let a = row(vec![cell(170.0, "2")]);
    let b = row(vec![cell(20.0, "1")]);
    let g = row_geometry(&[(0, &a), (1, &b)], 4, ShapeCx::COLUMNS, 0.0).unwrap();
    assert_eq!(g[0].height, 170.0);
    assert_eq!(g[0].cells[0].span, 1);
    assert!(g[0].cells[0].expandable);
    assert_eq!(g[1].height, 20.0);
}

#[test]
fn spanning_cell_growth_encompasses_only_its_originating_nonempty_row() {
    let a = row(vec![cell(180.0, "1"), cell(200.0, "2")]);
    let b = row(vec![cell(0.0, "1")]);
    let g = row_geometry(&[(0, &a), (0, &b)], 4, ShapeCx::COLUMNS, 0.0).unwrap();
    assert_eq!(g[0].height, 200.0);
    assert_eq!(g[1].height, 0.0);
    assert!(g[0].cells[1].expandable);
}

#[test]
fn spanning_populated_rows_keeps_independent_track_sizes() {
    let a = row(vec![cell(180.0, "1"), cell(170.0, "0")]);
    let b = row(vec![cell(20.0, "1")]);
    let g = row_geometry(&[(0, &a), (0, &b)], 4, ShapeCx::COLUMNS, 0.0).unwrap();
    assert_eq!(g[0].height, 180.0);
    assert_eq!(g[1].height, 20.0);
    assert_eq!(g[0].cells[1].span, 2);
    assert!(!g[0].cells[1].expandable);
}

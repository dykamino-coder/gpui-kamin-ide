//! Записи групп строк и слоёв: непрерывные группы и края вне сетки.

use super::{collect_group_edges, column_bands};
use crate::dom::Node;
use crate::layout::table::anon::{RowCarry, TableRow, anon_element};
use crate::style::values::value::Color;

#[test]
fn repeated_group_after_ungrouped_row_starts_a_new_segment() {
    let group = anon_element("tbody", vec![]);
    let mut elements: Vec<_> = (0..4).map(|_| anon_element("tr", vec![])).collect();
    for (i, row) in elements.iter_mut().enumerate() {
        row.node_id = i as u64 + 1;
    }
    let rows: Vec<_> = elements
        .iter()
        .enumerate()
        .map(|(i, element)| TableRow {
            element,
            carry: RowCarry {
                group: (i != 2).then_some(&group),
                ..Default::default()
            },
        })
        .collect();
    let mut edges = Default::default();
    collect_group_edges(&rows, &mut edges);
    assert!(edges[&1].first && !edges[&1].last);
    assert!(!edges[&2].first && edges[&2].last);
    assert!(!edges.contains_key(&3));
    assert!(edges[&4].first && edges[&4].last);
    assert!(std::ptr::eq(edges[&4].element, &group));
}

#[test]
fn column_records_retain_elements_beyond_grid_without_paint_probes() {
    let mut col = anon_element("col", vec![]);
    col.attrs = vec![("span".into(), "3".into())];
    col.style.cascaded_mut().background = Some(Color {
        r: 0.0,
        g: 0.5,
        b: 0.0,
        a: 1.0,
    });
    let table = anon_element("table", vec![Node::Element(col)]);
    let cx = crate::layout::cx::LayoutCx::default();
    let mut under = Vec::new();
    let layers = column_bands(&table, 1, &mut under, true, &cx);
    assert_eq!(layers.columns.len(), 3);
    assert!(layers.columns.iter().all(|layer| layer.element.is_some()));
    assert!(layers.columns[0].rects.is_some());
    assert!(
        layers.columns[1..]
            .iter()
            .all(|layer| layer.rects.is_none())
    );
    assert_eq!(under.len(), 1);
    let mut under = Vec::new();
    let layers = column_bands(&table, 1, &mut under, false, &cx);
    assert!(layers.columns.iter().all(|layer| layer.rects.is_none()));
    assert!(under.is_empty());
}

//! Охваты ячейки: граница группы и независимые причины обрезки.

use super::{CellSlot, cell_spans};
use crate::layout::table::anon::anon_element;
use crate::style::computed::Overflow;

#[test]
fn empty_cells_use_direct_text_facts_without_losing_css_spaces() {
    use crate::dom::{BoxTextFacts, BoxTreeSummary, Node};
    for (text, empty) in [
        ("", true),
        (" \t\r\n", true),
        ("\u{a0}", false),
        ("x", false),
    ] {
        let cell = anon_element("td", vec![Node::Text(text.into())]);
        assert_eq!(super::empty_cell_text(&cell), empty);
        cell.children.publish_box_summary(BoxTreeSummary {
            text: BoxTextFacts::text(text),
            ..Default::default()
        });
        assert_eq!(super::empty_cell_text(&cell), empty);
    }
    let cell = anon_element("td", vec![Node::Element(anon_element("span", vec![]))]);
    assert!(!super::empty_cell_text(&cell));
    cell.children.publish_box_summary(BoxTreeSummary {
        text: BoxTextFacts {
            elements: true,
            ..Default::default()
        },
        ..Default::default()
    });
    assert!(!super::empty_cell_text(&cell));
}

#[test]
fn row_group_limit_and_collapsed_columns_keep_distinct_flags() {
    let mut cell = anon_element("td", vec![]);
    cell.attrs = vec![
        ("colspan".into(), "2".into()),
        ("rowspan".into(), "0".into()),
    ];
    let spans = cell_spans(
        CellSlot {
            element: &cell,
            column: 1,
            colspan: 2,
            rowspan: 3,
        },
        &[true, false, false],
        &cell,
    );
    assert_eq!((spans.span_cols, spans.span_rows), (2, 3));
    assert!(!spans.spans_collapsed);
    assert!(!spans.clipped);
    let spans = cell_spans(
        CellSlot {
            element: &cell,
            column: 1,
            colspan: 2,
            rowspan: 3,
        },
        &[false, true, false],
        &cell,
    );
    assert!(spans.spans_collapsed);
    assert!(spans.clipped);
    cell.style.used_mut().reset_box_layout.overflow_y = Some(Overflow::Hidden);
    let spans = cell_spans(
        CellSlot {
            element: &cell,
            column: 1,
            colspan: 2,
            rowspan: 3,
        },
        &[false; 3],
        &cell,
    );
    assert!(!spans.spans_collapsed);
    assert!(spans.clipped);
}

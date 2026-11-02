//! Охваты ячейки: граница группы и независимые причины обрезки.

use super::{CellSlot, cell_spans};
use crate::layout::table::anon::anon_element;
use crate::style::computed::Overflow;

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

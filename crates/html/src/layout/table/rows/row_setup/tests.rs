//! Охваты ячейки: граница группы и независимые причины обрезки.

use super::cell_spans;
use crate::layout::table::anon::anon_element;
use crate::style::computed::Overflow;

#[test]
fn row_group_limit_and_collapsed_columns_keep_distinct_flags() {
    let mut cell = anon_element("td", vec![]);
    cell.attrs = vec![
        ("colspan".into(), "2".into()),
        ("rowspan".into(), "0".into()),
    ];
    let spans = cell_spans(2, &[5, 3], &[true, false, false], 1, &cell);
    assert_eq!((spans.span_cols, spans.span_rows), (2, 3));
    assert!(!spans.spans_collapsed);
    assert!(!spans.clipped);
    let spans = cell_spans(2, &[5, 3], &[false, true, false], 1, &cell);
    assert!(spans.spans_collapsed);
    assert!(spans.clipped);
    cell.style.used_mut().overflow_y = Some(Overflow::Hidden);
    let spans = cell_spans(2, &[5, 3], &[false; 3], 1, &cell);
    assert!(!spans.spans_collapsed);
    assert!(spans.clipped);
}

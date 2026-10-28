//! Ряды под правилом «not freely fragmentable» (css-tables-3 §breaking-rules): только ряды
//! после первого, без ячеек, тянущихся ниже них.

use super::*;
use crate::layout::fragment::types::RowKeep;

fn kept(html: &str) -> Vec<RowKeep> {
    let (b, _) = table(html);
    b.breaks(100.0, &frame(key(&b), &[])).table_rows
}

#[test]
fn rows_after_the_first_are_candidates_with_their_spacing_boundary() {
    let row = |prev, open, start, end| RowKeep {
        prev,
        open,
        start,
        end,
    };
    assert_eq!(
        kept(&rows(&["", "", ""], "x")),
        [row(12.0, 30.0, 32.0, 60.0), row(32.0, 60.0, 62.0, 88.0)]
    );
}

#[test]
fn a_row_spanned_into_the_next_one_is_freely_fragmentable() {
    // Ячейка второго ряда тянется в третий: второй ряд рвётся свободно, третий —
    // нет (охват в нём кончается).
    let cell = |span: &str, s: &str| {
        format!("<td style=\"display:table-cell\" rowspan=\"{span}\">{s}</td>")
    };
    let tr = |cells: String| format!("<tr style=\"display:table-row\">{cells}</tr>");
    let html = format!(
        "<table style=\"display:table\">{}{}{}</table>",
        tr(cell("1", "a")),
        tr(cell("2", "b")),
        tr(cell("1", "c"))
    );
    let starts: Vec<_> = kept(&html).iter().map(|r| r.start).collect();
    assert_eq!(starts, [62.0]);
}

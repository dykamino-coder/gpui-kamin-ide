//! Вложенный многоколоночник в записи главной копии внешней стопки: строка выше ряда
//! вложенного многоколоночника рисуется только в фрагментаинере, где она начинается,
//! переполняя его (css-break-3 §4.4: строка неделима; Blink: строка не переходит
//! в следующий фрагментаинер).

use super::super::quads;
use super::is;

/// Внешние колонки 100px, ряд вложенного многоколоночника — 100px, строка — 200px
/// (nested-with-too-tall-line).
fn nested(line: &str) -> String {
    format!(
        "<style>body{{margin:0}}</style>\
<div style='columns:2;gap:0;width:200px;height:100px;column-fill:auto'>\
<div style='columns:2;gap:0;height:200px;line-height:200px'>{line}</div></div>"
    )
}

/// Зелёные квады: ничего в колонке 2 внешней стопки, в колонке 1 — ниже `reach`.
fn only_where_it_starts(html: &str, reach: f32) {
    let found = quads(html, 400., 400.);
    let green: Vec<_> = found
        .iter()
        .filter(|(_, c)| is(Some(*c), 0x00ff00))
        .collect();
    let leaked = green.iter().any(|(b, _)| f32::from(b.origin.x) >= 99.);
    assert!(!leaked, "line leaks into the next outer column: {found:?}");
    let bottom = (green.iter()).fold(0., |m: f32, (b, _)| m.max(f32::from(b.bottom())));
    assert!(bottom >= reach, "line cut at the column edge: {found:?}");
}

#[test]
fn text_line_taller_than_its_nested_row_paints_only_where_it_starts() {
    // Подложка текста стоит посередине строки 200px и переходит край колонки 100px.
    only_where_it_starts(
        &nested("<span style='background:#00ff00'>PASS</span>"),
        101.,
    );
}

#[test]
fn atomic_line_taller_than_its_nested_row_paints_only_where_it_starts() {
    only_where_it_starts(
        &nested(
            "<span style='display:inline-block;vertical-align:top;width:20px;height:200px;\
background:#00ff00'></span>",
        ),
        199.,
    );
}

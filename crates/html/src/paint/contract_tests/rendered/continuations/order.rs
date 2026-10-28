//! Порядок краски продолжений: absolute из одной колонки рисуется в фазе
//! позиционированных своего контекста наложения, то есть поверх нормального потока
//! всех колонок, а не только своей (CSS 2.1 Appendix E).

use super::super::{quads, top};
use super::is;

fn columns(container: &str, height: u32) -> String {
    format!(
        "<style>body{{margin:0}}</style>\
<div style='columns:2;height:100px;width:100px;column-gap:0;column-fill:auto;position:relative'>\
<div style='{container}height:200px'>\
<div style='position:absolute;width:100px;height:{height}px;background:#00ff00'></div>\
<div style='width:50px;height:200px;background:#ff0000'></div></div></div>"
    )
}

#[test]
fn absolute_box_from_the_first_column_paints_over_the_next_column_flow() {
    for container in ["", "display:grid;"] {
        for height in [100, 150] {
            let found = quads(&columns(container, height), 200., 200.);
            for x in [25., 75.] {
                assert!(
                    is(top(&found, x, 50.), 0x00ff00),
                    "{container} {height} ({x}, 50): {found:?}"
                );
            }
        }
    }
}

#[test]
fn continuations_at_fractional_column_offsets_leave_no_device_pixel_between_columns() {
    // Thirds of 100px: each box snaps its own edges where it lands, like a box
    // laid out in that column, so no pixel of the red background shows.
    let found = quads(
        "<style>body{margin:0}</style>\
<div style='columns:3;height:100px;width:100px;column-gap:0;column-fill:auto;background:#ff0000'>\
<div style='position:relative;display:grid;grid-template-rows:100px 100px 100px'>\
<div style='background:#00ff00'>\
<div style='position:absolute;background:#00ff00;grid-row-start:2;inset:0'></div>\
<div style='position:absolute;background:#00ff00;grid-row-start:3;inset:0'></div>\
</div></div></div>",
        200.,
        200.,
    );
    for x in [1., 33.2, 33.8, 66.2, 66.8, 99.9] {
        assert!(is(top(&found, x, 50.), 0x00ff00), "({x}, 50): {found:?}");
    }
}

#[test]
fn box_in_a_table_fragment_is_cut_by_its_column_not_by_the_repeated_footer() {
    // css-break forced-break-before-repeated-footer-003: the box lies in the
    // first fragment of its table, beside and over the repeated footer band.
    let found = quads(
        "<style>body{margin:0}</style>\
<div style='columns:4;gap:0;column-fill:auto;width:100px;height:100px;background:#ff0000'>\
<div style='position:relative;display:table;width:100%'>\
<div style='position:absolute;top:70px;left:25px;width:25px;height:30px;background:#00ff00'></div>\
<div style='display:table-row;break-after:column'><div style='height:130px;background:#00ff00'></div></div>\
<div style='height:160px;background:#00ff00'></div>\
<div style='display:table-footer-group;break-inside:avoid'><div style='height:20px;background:#00ff00'></div></div>\
</div></div>",
        200.,
        200.,
    );
    for y in [72., 85., 98.] {
        assert!(is(top(&found, 37., y), 0x00ff00), "(37, {y}): {found:?}");
    }
}

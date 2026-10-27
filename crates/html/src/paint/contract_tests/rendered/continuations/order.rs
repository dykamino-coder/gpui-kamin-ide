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

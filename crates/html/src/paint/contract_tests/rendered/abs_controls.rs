//! Absolutely positioned form controls are out of flow (CSS 2.1 §9.3.1): they
//! paint at their insets and take no room in the line where they are written.

use super::continuations::is;
use super::{quads, top};

#[test]
fn absolute_checkbox_paints_at_its_insets_and_leaves_the_flow() {
    let html = "<style>body{margin:0}</style>\
                <input type=checkbox checked style='accent-color:#00ff00;position:absolute;left:200px;top:50px'>\
                <div style='margin-top:30px;height:20px;background:#0000ff'></div>";
    let found = quads(html, 400., 200.);
    assert!(
        is(top(&found, 206., 56.), 0x00ff00),
        "checkbox at its insets"
    );
    assert!(
        !is(top(&found, 6., 6.), 0x00ff00),
        "nothing at the static position"
    );
    assert!(
        is(top(&found, 25., 35.), 0x0000ff),
        "the block starts at its margin"
    );
    assert!(
        !is(top(&found, 25., 55.), 0x0000ff),
        "no line box above the block"
    );
}

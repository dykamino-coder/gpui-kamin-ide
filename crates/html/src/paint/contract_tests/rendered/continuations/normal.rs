//! Обычный поток, нарезанный колонками и нарисованный записью главной копии: каждый
//! кусок виден и попадается в своей колонке, и только своим срезом.

use super::super::{cursors, quads, top};
use super::is;
use gpui::CursorStyle;

const SPLIT: &str = "<style>body{margin:0}</style>\
<div style='columns:2;gap:0;width:100px;height:100px;column-fill:auto'>\
<div style='cursor:pointer'>\
<div style='height:100px;background:#00ff00'></div>\
<div style='height:60px;background:#0000ff'></div></div></div>";

#[test]
fn replayed_pieces_paint_and_hit_in_their_own_columns() {
    let found = quads(SPLIT, 400., 200.);
    assert!(is(top(&found, 25., 50.), 0x00ff00), "first piece");
    assert!(is(top(&found, 75., 30.), 0x0000ff), "second piece");
    assert!(!is(top(&found, 75., 80.), 0x0000ff), "below the content");
    assert!(
        top(&found, 25., 130.).is_none(),
        "nothing at the source below the column"
    );
    let at = cursors(
        SPLIT,
        false,
        400.,
        200.,
        &[(25., 50.), (75., 30.), (25., 130.)],
    );
    assert_eq!(at[0], CursorStyle::PointingHand, "first piece");
    assert_eq!(at[1], CursorStyle::PointingHand, "second piece");
    assert_eq!(
        at[2],
        CursorStyle::Arrow,
        "source position below the column"
    );
}

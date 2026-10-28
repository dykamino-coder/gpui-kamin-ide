//! Клик по продолжению попадает туда, где оно нарисовано: другая колонка, матрица копии,
//! другой лист; на прежнем месте источника — нет.

use super::super::{cursors, printed, quads, top};
use super::is;
use gpui::CursorStyle;

const POINTER: &str = "<style>body{margin:0}</style>\
<div style='columns:2;gap:0;width:100px;height:100px;column-fill:auto'>\
<div style='position:relative;height:200px'>\
<div style='position:absolute;top:50px;width:100%;height:100px;cursor:pointer;background:#0000ff'></div>\
</div></div>";

#[test]
fn continuation_in_the_next_column_is_hit_there_and_not_at_its_source() {
    let at = cursors(
        POINTER,
        false,
        400.,
        200.,
        &[(25., 75.), (75., 25.), (25., 125.)],
    );
    assert_eq!(at[0], CursorStyle::PointingHand, "first fragment");
    assert_eq!(at[1], CursorStyle::PointingHand, "continuation in column 2");
    assert_eq!(at[2], CursorStyle::Arrow, "unfragmented source position");
}

#[test]
fn continuation_under_a_transformed_copy_is_painted_and_hit_through_its_matrix() {
    // The copy of the transformed box in column 2 scales its fragment about the
    // fragment's bottom: the continuation there (source 50..75) shows at 50..100.
    let html = "<style>body{margin:0}</style>\
<div style='columns:2;gap:0;width:100px;height:100px;column-fill:auto'>\
<div style='height:50px'></div>\
<div style='transform:scaleY(2);transform-origin:bottom;width:50px;height:100px'>\
<div><div style='position:absolute;top:100px;width:50px;height:25px;cursor:pointer;background:#0000ff'></div></div>\
</div></div>";
    let found = quads(html, 400., 200.);
    assert!(is(top(&found, 75., 90.), 0x0000ff), "scaled continuation");
    assert!(!is(top(&found, 75., 45.), 0x0000ff));
    let at = cursors(html, false, 400., 200., &[(75., 90.), (75., 45.)]);
    assert_eq!(at[0], CursorStyle::PointingHand, "hit where it is painted");
    assert_eq!(at[1], CursorStyle::Arrow);
}

#[test]
fn continuation_on_the_next_page_is_painted_and_hit_there() {
    let html = "<style>body{margin:0}</style>\
<div style='position:relative'>\
<div style='position:absolute;top:150px;width:100px;height:200px;cursor:pointer;background:#0000ff'></div></div>";
    let found = printed(html, 800., 600.);
    let blue: Vec<_> = found
        .iter()
        .filter(|(_, c)| c.to_rgb() == gpui::rgb(0x0000ff))
        .map(|(b, _)| *b)
        .collect();
    assert!(blue.len() >= 2, "fragments on two pages: {blue:?}");
    let last = blue
        .iter()
        .max_by(|a, b| f32::from(a.origin.y).total_cmp(&f32::from(b.origin.y)))
        .unwrap();
    let center = last.center();
    let at = cursors(
        html,
        true,
        800.,
        600.,
        &[(center.x.into(), center.y.into())],
    );
    assert_eq!(at[0], CursorStyle::PointingHand);
}

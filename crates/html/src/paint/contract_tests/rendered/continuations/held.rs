//! Семьи, которые прежде держали legacy-копии (anchor, процентный статический abs,
//! transform дальше переноса, коробка с CB вне multicol), и статические места в колонках.

use super::super::{quads, top};
use super::{any, is, opaque};

#[test]
fn box_whose_parent_is_its_containing_block_continues_from_its_static_position() {
    let found = quads(
        "<style>body{margin:0}</style>\
<div style='columns:2;gap:0;width:100px;height:100px;column-fill:auto'>\
<div style='position:relative'><div style='height:150px'></div>\
<div style='position:absolute;width:50px;height:50px;background:#0000ff'></div></div></div>",
        400.,
        200.,
    );
    assert!(is(top(&found, 75., 75.), 0x0000ff), "{found:?}");
    assert!(!any(&found, 0x0000ff) || !is(top(&found, 25., 25.), 0x0000ff));
}

#[test]
fn box_after_a_spanner_takes_its_static_position_in_its_row() {
    // The containing block breaks across the spanner; the box follows the two
    // blocks of the second row, so it starts in that row's second column.
    let found = quads(
        "<style>body{margin:0}</style>\
<div style='width:200px;columns:2;gap:0;position:relative'>\
<div style='position:relative'><div style='contain:size;height:80px'></div>\
<div style='column-span:all;height:20px'></div>\
<div style='contain:size;height:100px'></div><div style='contain:size;height:100px'></div>\
<div style='position:absolute;width:100%;height:100px;background:#0000ff'></div></div></div>",
        400.,
        400.,
    );
    let blue: Vec<_> = found
        .iter()
        .filter(|(_, c)| opaque(*c) == 0x0000ff)
        .map(|(b, _)| *b)
        .collect();
    assert!(
        !blue.is_empty() && blue.iter().all(|b| f32::from(b.origin.x) >= 100.),
        "{blue:?}"
    );
}

#[test]
fn transformed_root_keeps_a_transform_origin_per_fragment() {
    // css-break transform-021: scaleY(2) about each fragment's own bottom. The
    // root's fragments in columns 2 and 3 grow above the columns; the nested
    // box continues into column 4 under a zero-height fragment's transform.
    let found = quads(
        "<style>body{margin:0}</style>\
<div style='display:flow-root;position:relative;width:100px;height:100px'>\
<div style='columns:4;margin-top:50px;gap:0;column-fill:auto;width:100px;height:50px'>\
<div style='position:relative;height:50px'>\
<div style='position:absolute;transform:scaleY(2);transform-origin:bottom;top:50px;width:100%;height:100px'>\
<div style='position:absolute;top:50px;width:100%;height:75px;background:#0000ff'></div>\
<div style='height:50px;background:#0000ff'></div></div></div></div></div>",
        400.,
        200.,
    );
    for (x, y) in [(37., 25.), (37., 75.), (62., 25.), (62., 75.), (87., 75.)] {
        assert!(is(top(&found, x, y), 0x0000ff), "({x}, {y}): {found:?}");
    }
}

#[test]
fn anchored_box_continues_from_its_settled_anchor_position() {
    // The box is placed under its anchor (top 70px) and breaks into column 2.
    let found = quads(
        "<style>body{margin:0}</style>\
<div style='columns:2;gap:0;width:100px;height:100px;column-fill:auto'>\
<div style='position:relative'><div style='height:50px'></div><div style='anchor-name:--a;height:20px'></div>\
<div style='position:absolute;position-anchor:--a;top:anchor(bottom);left:0;width:50px;height:100px;background:#0000ff'></div>\
<div style='height:130px'></div></div></div>",
        400.,
        200.,
    );
    for (x, y) in [(25., 85.), (75., 35.)] {
        assert!(is(top(&found, x, y), 0x0000ff), "({x}, {y}): {found:?}");
    }
    assert!(!is(top(&found, 25., 60.), 0x0000ff), "{found:?}");
    assert!(!is(top(&found, 75., 85.), 0x0000ff), "{found:?}");
}

#[test]
fn fixed_box_nested_in_a_continued_root_takes_its_static_position_from_the_columns() {
    // css-break out-of-flow-in-multicolumn-026: the viewport is the fixed box's
    // containing block; its static position lies in the first column.
    let found = quads(
        "<style>body{margin:0}</style>\
<div style='columns:2;column-fill:auto;gap:0;height:100px;width:100px;background:#ff0000'>\
<div style='position:relative'><div style='position:absolute'>\
<div style='position:fixed;width:100px;height:100px;background:#00ff00'></div></div></div></div>",
        400.,
        200.,
    );
    for (x, y) in [(5., 5.), (95., 95.)] {
        assert!(is(top(&found, x, y), 0x00ff00), "({x}, {y}): {found:?}");
    }
    let green = found.iter().filter(|(_, c)| opaque(*c) == 0x00ff00).count();
    assert_eq!(green, 1, "painted once: {found:?}");
}

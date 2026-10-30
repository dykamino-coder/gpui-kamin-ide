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

/// Ячейка 2 начинается монолитом выше колонки, за ним — блок; ячейка 1 тянется на весь ряд.
const MONOLITH: &str = "<style>body{margin:0}</style><div style='columns:2;column-fill:auto;gap:0;width:100px;height:100px;background:#ff0000'><div style='display:table;width:100%;height:200px'><div style='display:table-cell;width:50%'><div style='height:100%;background:#00ff00'></div></div><div style='display:table-cell;width:50%'><div style='contain:size;height:130px'><div style='height:130px;background:#ff00ff'></div></div><div style='height:100px;background:#0000ff'></div></div></div></div>";

#[test]
fn monolith_overflows_only_its_own_column_and_the_cell_resumes_at_the_next_top() {
    let found = quads(MONOLITH, 400., 300.);
    assert!(is(top(&found, 12., 50.), 0x00ff00), "cell 1, column 1");
    assert!(is(top(&found, 37., 50.), 0xff00ff), "monolith, column 1");
    assert!(
        is(top(&found, 37., 115.), 0xff00ff),
        "monolith overflow below its own column"
    );
    assert!(
        is(top(&found, 62., 95.), 0x00ff00),
        "cell 1 continues to 200"
    );
    assert!(
        is(top(&found, 87., 5.), 0x0000ff),
        "cell 2 resumes at the top of column 2"
    );
    assert!(is(top(&found, 87., 95.), 0x0000ff), "row height 200");
    assert!(
        !found
            .iter()
            .any(|(b, c)| f32::from(b.origin.x) >= 50. && super::opaque(*c) == 0xff00ff),
        "no monolith in column 2"
    );
    assert_eq!(top(&found, 125., 50.), None, "no overflow column");
}

#[test]
fn replayed_flex_items_paint_from_their_own_fragments() {
    let html = "<style>body{margin:0}</style><div style='columns:2;gap:0;width:100px;height:100px;column-fill:auto'><div style='display:flex;flex-direction:column'><div style='height:70px;background:#00ff00'></div><div style='height:70px;background:#0000ff'></div></div></div>";
    let found = quads(html, 400., 300.);
    assert!(is(top(&found, 25., 35.), 0x00ff00), "first item");
    assert!(is(top(&found, 25., 85.), 0x0000ff), "second item, column 1");
    assert!(is(top(&found, 75., 20.), 0x0000ff), "second item, column 2");
    assert!(
        !is(top(&found, 75., 60.), 0x0000ff),
        "below the second item"
    );
    assert!(
        !is(top(&found, 75., 20.), 0x00ff00),
        "first item stays in column 1"
    );
}

#[test]
fn replayed_grid_items_paint_in_their_own_windows() {
    // The second row's item is unbreakable: the grid pushes it to column 2.
    let html = "<style>body{margin:0}</style><div style='columns:2;gap:0;width:100px;height:100px;column-fill:auto'><div style='display:grid'><div style='height:70px;background:#00ff00'></div><div style='contain:size;height:70px;background:#0000ff'></div></div></div>";
    let found = quads(html, 400., 300.);
    assert!(is(top(&found, 25., 35.), 0x00ff00), "first item");
    assert!(
        !is(top(&found, 25., 85.), 0x0000ff),
        "pushed item not in column 1"
    );
    assert!(
        is(top(&found, 75., 35.), 0x0000ff),
        "pushed item at the top of column 2"
    );
}

#[test]
fn replayed_shadow_stays_with_the_fragment_of_its_box() {
    // The first box ends exactly at the column break: its shadow belongs to column 1
    // (below the column, at its genuine bottom edge), not to the top of column 2.
    let html = "<style>body{margin:0}</style><div style='columns:2;gap:0;width:100px;height:100px;column-fill:auto'><div><div style='height:100px;background:#00ff00;box-shadow:0 10px 0 #0000ff'></div><div style='height:60px'></div></div></div>";
    let found = quads(html, 400., 300.);
    assert!(is(top(&found, 25., 50.), 0x00ff00), "first box");
    assert!(
        is(top(&found, 25., 105.), 0x0000ff),
        "shadow below column 1"
    );
    assert!(
        !is(top(&found, 75., 5.), 0x0000ff),
        "no shadow at the top of column 2"
    );
}

#[test]
fn replayed_transform_uses_the_reference_box_of_each_fragment() {
    // scaleY(0.5) about the centre of each fragment's reference box (css-break-3 §5.5):
    // column 1 holds [0, 100) scaled to [25, 75), column 2 holds [100, 160) scaled to
    // [15, 45).
    let html = "<style>body{margin:0}</style><div style='columns:2;gap:0;width:100px;height:100px;column-fill:auto'><div><div style='transform:scaleY(0.5);height:160px;background:#00ff00'></div></div></div>";
    let found = quads(html, 400., 300.);
    assert!(is(top(&found, 25., 30.), 0x00ff00), "column 1, inside");
    assert!(!is(top(&found, 25., 85.), 0x00ff00), "column 1, below");
    assert!(is(top(&found, 75., 20.), 0x00ff00), "column 2, inside");
    assert!(!is(top(&found, 75., 50.), 0x00ff00), "column 2, below");
}

#[test]
fn replayed_shadow_of_a_flex_item_follows_the_item_fragments() {
    // The second item starts in column 1 and ends in column 2: its shadow is kept at its
    // genuine bottom edge in column 2 and is not painted below column 1.
    let html = "<style>body{margin:0}</style><div style='columns:2;gap:0;width:100px;height:100px;column-fill:auto'><div style='display:flex;flex-direction:column'><div style='height:70px;background:#00ff00'></div><div style='height:70px;background:#0000ff;box-shadow:0 10px 0 #ff00ff'></div></div></div>";
    let found = quads(html, 400., 300.);
    assert!(is(top(&found, 25., 85.), 0x0000ff), "second item, column 1");
    assert!(is(top(&found, 75., 20.), 0x0000ff), "second item, column 2");
    assert!(
        is(top(&found, 75., 45.), 0xff00ff),
        "shadow at the item's bottom edge"
    );
    assert!(
        !is(top(&found, 25., 105.), 0xff00ff),
        "no shadow below column 1"
    );
}

#[test]
fn replayed_row_rules_are_cut_per_fragment() {
    // CSS Gaps 1 §fragmentation: the gutter at the column break disappears with its rule.
    let html = "<style>body{margin:0}</style><div style='columns:2;gap:0;width:200px;height:100px;column-fill:auto'><div style='display:grid;grid-template-rows:40px 40px 40px;row-gap:10px;row-rule:4px solid #0000ff'><div></div><div></div><div></div></div></div>";
    let found = quads(html, 400., 300.);
    assert!(is(top(&found, 50., 45.), 0x0000ff), "rule inside column 1");
    assert!(
        !is(top(&found, 50., 95.), 0x0000ff),
        "no rule in the gutter at the break"
    );
}

#[test]
fn replayed_relative_box_paints_in_its_own_columns() {
    let html = "<style>body{margin:0}</style><div style='columns:2;gap:0;width:100px;height:100px;column-fill:auto'><div style='position:relative;left:10px'><div style='height:100px;background:#00ff00'></div><div style='height:60px;background:#0000ff'></div></div></div>";
    let found = quads(html, 400., 300.);
    assert!(is(top(&found, 30., 50.), 0x00ff00), "first piece");
    assert!(is(top(&found, 80., 30.), 0x0000ff), "second piece");
    assert!(
        top(&found, 30., 130.).is_none(),
        "nothing at the source below the column"
    );
}

/// Внепоточный корень в ребёнке, нарисованном записью: после потока всех кусков.
const OOF_OVER_FLOW: &str = "<style>body{margin:0}</style><div style='columns:2;height:100px;width:100px;gap:0;column-fill:auto;position:relative'><div style='display:grid;height:200px'><div style='position:absolute;width:100px;height:100px;background:#00ff00'></div><div style='width:50px;height:200px;background:#ff0000'></div></div></div>";

#[test]
fn replayed_child_paints_its_out_of_flow_root_after_the_flow_of_every_piece() {
    let found = quads(OOF_OVER_FLOW, 400., 200.);
    assert!(is(top(&found, 25., 50.), 0x00ff00), "column 1");
    assert!(
        is(top(&found, 75., 50.), 0x00ff00),
        "over the flow of column 2"
    );
}

/// Вложенный многоколоночник со своим внепоточным корнем внутри внепоточного корня
/// внешнего: его запись внутри нативного источника внешней стопки.
const NESTED_IN_SOURCE: &str = "<style>body{margin:0}</style><div style='columns:2;gap:0;column-fill:auto;width:100px;height:100px;background:#ff0000'><div style='position:relative'><div style='position:absolute;columns:2;gap:0;column-fill:auto;width:100%;height:100px'><div style='position:relative'><div style='position:absolute;width:100%;height:400px;background:#00ff00'></div></div></div></div></div>";

#[test]
fn nested_multicol_records_its_pieces_inside_the_outer_native_source() {
    let found = quads(NESTED_IN_SOURCE, 400., 200.);
    for x in [12., 37., 62., 87.] {
        assert!(is(top(&found, x, 50.), 0x00ff00), "column at {x}");
    }
}

#[test]
fn replayed_table_rows_resume_under_the_repeated_header() {
    let html = "<style>body{margin:0}td{padding:0}</style>\
<div style='columns:2;gap:0;width:100px;height:100px;column-fill:auto'>\
<table style='width:100%;border-spacing:0'>\
<thead><tr><td style='height:20px;background:#0000ff'></td></tr></thead>\
<tr><td style='height:70px;background:#00ff00'></td></tr>\
<tr><td style='height:70px;background:#ff00ff'></td></tr></table></div>";
    let found = quads(html, 400., 300.);
    assert!(is(top(&found, 25., 10.), 0x0000ff), "header, column 1");
    assert!(is(top(&found, 25., 50.), 0x00ff00), "row 1, column 1");
    assert!(is(top(&found, 75., 10.), 0x0000ff), "repeated header");
    assert!(
        is(top(&found, 75., 50.), 0xff00ff),
        "row 2 under the header"
    );
    assert!(!is(top(&found, 75., 95.), 0xff00ff), "row 2 ends at 90");
}

#[test]
fn replayed_collapsed_table_edges_end_at_the_fragmentation_cut() {
    let html = "<style>body{margin:0}</style>\
<div style='columns:2;gap:0;width:200px;height:100px;column-fill:auto'>\
<table style='width:100%;border-collapse:collapse'>\
<tr><td style='height:180px;padding:0;border:10px solid #0000ff'></td></tr></table></div>";
    let found = quads(html, 400., 300.);
    let wide = |(b, c): &&(gpui::Bounds<gpui::Pixels>, gpui::Hsla)| {
        is(Some(*c), 0x0000ff) && f32::from(b.size.width) > 50.
    };
    let edges: Vec<_> = found.iter().filter(wide).collect();
    // Верхняя кромка — в колонке 1, нижняя — в колонке 2; на разрезе кромки нет.
    assert!(
        edges
            .iter()
            .any(|(b, _)| f32::from(b.origin.x) < 50. && f32::from(b.top()) < 10.),
        "top edge in column 1: {found:?}"
    );
    assert!(
        edges
            .iter()
            .any(|(b, _)| f32::from(b.origin.x) >= 99. && f32::from(b.top()) > 50.),
        "bottom edge in column 2: {found:?}"
    );
    assert!(
        !edges
            .iter()
            .any(|(b, _)| f32::from(b.origin.x) >= 99. && f32::from(b.top()) < 10.),
        "no edge at the top of column 2: {found:?}"
    );
}

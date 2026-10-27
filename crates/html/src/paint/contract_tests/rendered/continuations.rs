//! Продолжения absolute/fixed в реальном документе: размещение, clip containing block
//! и общий эффект предка с копией нормального потока.

use super::{cursors, grouped, printed, quads, top};
use gpui::{CursorStyle, Hsla, Pixels};

fn is(color: Option<Hsla>, rgb: u32) -> bool {
    color.is_some_and(|c| c.to_rgb() == gpui::rgb(rgb))
}

/// The colour without alpha as 0xRRGGBB.
fn opaque(c: Hsla) -> u32 {
    let c = c.to_rgb();
    let byte = |v: f32| (v * 255.).round() as u32;
    byte(c.r) << 16 | byte(c.g) << 8 | byte(c.b)
}

fn any(found: &[(gpui::Bounds<Pixels>, Hsla)], rgb: u32) -> bool {
    found.iter().any(|(_, c)| c.to_rgb() == gpui::rgb(rgb))
}

const COLUMNS: &str = "<style>body{margin:0}</style>\
<div style='columns:2;gap:0;width:100px;height:100px;column-fill:auto'>\
<div style='position:relative;height:200px;CLIP'>\
<div style='position:absolute;width:100%;height:300px'>\
<div style='height:200px;background:#00ff00'></div>\
<div style='height:200px;background:#0000ff'></div></div></div></div>";

#[test]
fn continuation_past_its_containing_block_reaches_the_overflow_column() {
    let found = quads(&COLUMNS.replace("CLIP", ""), 400., 200.);
    assert!(is(top(&found, 25., 50.), 0x00ff00));
    assert!(is(top(&found, 75., 50.), 0x00ff00));
    // The third fragment (source 200..300) lands in the first overflow column.
    assert!(is(top(&found, 125., 50.), 0x0000ff));
}

#[test]
fn containing_block_overflow_clip_cuts_continuations_before_placement() {
    let found = quads(&COLUMNS.replace("CLIP", "overflow-y:clip"), 400., 200.);
    assert!(is(top(&found, 25., 50.), 0x00ff00));
    assert!(is(top(&found, 75., 50.), 0x00ff00));
    assert!(!any(&found, 0x0000ff), "clipped continuation painted");
    assert_eq!(top(&found, 125., 50.), None);
}

#[test]
fn continuation_replays_inside_the_copy_group_of_a_semi_transparent_ancestor() {
    // The column copy's single instance of the ancestor's effects holds both its
    // normal content and the continuation it replays, in CSS paint order. (Copies
    // apply opacity per primitive, the same as main; the replay gets it once.)
    let found = grouped(
        "<style>body{margin:0}</style>\
<div style='columns:2;gap:0;width:100px;height:100px;column-fill:auto'>\
<div style='opacity:0.5;position:relative;height:200px'>\
<div style='height:200px;background:#ff0000'></div>\
<div style='position:absolute;top:50px;width:100%;height:100px;background:#0000ff'></div>\
</div></div>",
        400.,
        200.,
    );
    let second = |rgb: u32| {
        found
            .iter()
            .filter(|(b, c, _)| opaque(*c) == rgb && f32::from(b.origin.x) >= 50.)
            .map(|(_, _, group)| *group)
            .collect::<Vec<_>>()
    };
    let (normal, oof) = (second(0xff0000), second(0x0000ff));
    assert_eq!(oof.len(), 1, "one continuation fragment in the second column");
    assert!(normal.contains(&oof[0]), "same group instance as the normal flow");
    // Paint order: the positioned box comes after the normal flow of its group.
    let order = |rgb: u32| {
        found
            .iter()
            .position(|(b, c, g)| {
                *g == oof[0] && opaque(*c) == rgb && f32::from(b.origin.x) >= 50.
            })
    };
    assert!(order(0xff0000) < order(0x0000ff));
    // The ancestor's opacity reaches both exactly once.
    let alpha = |rgb: u32| {
        found
            .iter()
            .find(|(b, c, _)| opaque(*c) == rgb && f32::from(b.origin.x) >= 50.)
            .map(|(_, c, _)| c.a)
    };
    assert_eq!(alpha(0xff0000), alpha(0x0000ff));
}

const POINTER: &str = "<style>body{margin:0}</style>\
<div style='columns:2;gap:0;width:100px;height:100px;column-fill:auto'>\
<div style='position:relative;height:200px'>\
<div style='position:absolute;top:50px;width:100%;height:100px;cursor:pointer;background:#0000ff'></div>\
</div></div>";

#[test]
fn continuation_in_the_next_column_is_hit_there_and_not_at_its_source() {
    let at = cursors(POINTER, false, 400., 200., &[(25., 75.), (75., 25.), (25., 125.)]);
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
    let last = blue.iter().max_by(|a, b| a.origin.y.partial_cmp(&b.origin.y).unwrap()).unwrap();
    let center = last.center();
    let at = cursors(html, true, 800., 600., &[(center.x.into(), center.y.into())]);
    assert_eq!(at[0], CursorStyle::PointingHand);
}

#[test]
fn box_whose_containing_block_is_outside_the_multicol_is_not_fragmented() {
    // The initial containing block lays the box out after the spanner: it stays
    // whole in place instead of continuing in the second column.
    let found = quads(
        "<style>body{margin:0}</style>\
<div style='columns:2;column-fill:auto;gap:0;width:200px'>\
<div style='column-span:all;height:10px'></div>\
<div style='position:absolute;width:100px;height:100px;background:#0000ff'></div>\
<div style='height:100px'></div></div>",
        400.,
        300.,
    );
    let blue: Vec<_> = found
        .iter()
        .filter(|(_, c)| opaque(*c) == 0x0000ff)
        .map(|(b, _)| *b)
        .collect();
    assert_eq!(blue.len(), 1, "painted once: {blue:?}");
    assert_eq!(f32::from(blue[0].origin.x), 0.);
    assert_eq!(f32::from(blue[0].origin.y), 10.);
    assert_eq!(f32::from(blue[0].size.height), 100.);
}

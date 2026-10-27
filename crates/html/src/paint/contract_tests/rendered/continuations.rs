//! Продолжения absolute/fixed в реальном документе: размещение, clip containing block
//! и общий эффект предка с копией нормального потока.

use super::{grouped, quads, top};
use gpui::{Hsla, Pixels};

pub(super) fn is(color: Option<Hsla>, rgb: u32) -> bool {
    color.is_some_and(|c| c.to_rgb() == gpui::rgb(rgb))
}

/// The colour without alpha as 0xRRGGBB.
pub(super) fn opaque(c: Hsla) -> u32 {
    let c = c.to_rgb();
    let byte = |v: f32| (v * 255.).round() as u32;
    byte(c.r) << 16 | byte(c.g) << 8 | byte(c.b)
}

pub(super) fn any(found: &[(gpui::Bounds<Pixels>, Hsla)], rgb: u32) -> bool {
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
    assert_eq!(
        oof.len(),
        1,
        "one continuation fragment in the second column"
    );
    assert!(
        normal.contains(&oof[0]),
        "same group instance as the normal flow"
    );
    // Paint order: the positioned box comes after the normal flow of its group.
    let order = |rgb: u32| {
        found
            .iter()
            .position(|(b, c, g)| *g == oof[0] && opaque(*c) == rgb && f32::from(b.origin.x) >= 50.)
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

#[test]
fn nested_box_above_its_containing_block_fragment_starts_in_the_earlier_column() {
    // The inner box ends 100px below its zero-height containing block at the
    // top of column 2: it fills column 1 and continues in column 2.
    let found = quads(
        "<style>body{margin:0}</style>\
<div style='columns:2;gap:0;width:100px;height:100px;column-fill:auto'>\
<div style='position:relative;height:100px;width:50px'>\
<div style='position:absolute;width:50px;bottom:0'>\
<div style='position:absolute;width:50px;bottom:-100px;background:#0000ff'>\
<div style='height:200px'></div></div></div></div></div>",
        400.,
        200.,
    );
    assert!(
        is(top(&found, 25., 50.), 0x0000ff),
        "first fragment in column 1"
    );
    assert!(
        is(top(&found, 75., 50.), 0x0000ff),
        "continuation in column 2"
    );
}

#[test]
fn continuation_keeps_the_inline_offsets_of_its_ancestors() {
    let found = quads(
        "<style>body{margin:0}</style>\
<div style='columns:2;gap:0;width:100px;height:100px;column-fill:auto'>\
<div style='margin-left:10px'><div style='position:relative;height:200px'>\
<div style='position:absolute;width:30px;height:200px;background:#0000ff'></div></div></div></div>",
        400.,
        200.,
    );
    assert!(is(top(&found, 15., 50.), 0x0000ff));
    assert!(!is(top(&found, 5., 50.), 0x0000ff));
    assert!(
        is(top(&found, 65., 50.), 0x0000ff),
        "continuation keeps the 10px offset"
    );
    assert!(!is(top(&found, 55., 50.), 0x0000ff));
}

#[test]
fn continuation_paints_its_gradient_background_in_every_fragment() {
    let found = quads(
        "<style>body{margin:0}</style>\
<div style='columns:2;gap:0;width:100px;height:100px;column-fill:auto'>\
<div style='position:relative;height:200px'>\
<div style='position:absolute;top:50px;width:50px;height:100px;\
background:repeating-linear-gradient(lime, lime 20px, blue 20px, blue 40px)'></div></div></div>",
        400.,
        200.,
    );
    let gradient = |x: f32, y: f32| top(&found, x, y).is_some_and(|c| c.s == 0. && c.l == 0.);
    assert!(gradient(25., 75.), "first fragment: {found:?}");
    assert!(gradient(75., 25.), "continuation: {found:?}");
}

#[test]
fn absolute_box_in_a_repeated_table_header_repeats_with_it() {
    let found = quads(
        "<style>body{margin:0}</style>\
<div style='columns:2;column-fill:auto;gap:0;width:100px;height:100px'>\
<div style='display:table;width:100%'>\
<div style='display:table-header-group;break-inside:avoid'>\
<div style='position:relative;contain:size;width:50px;height:20px'>\
<div style='position:absolute;width:50px;height:20px;background:#0000ff'></div></div></div>\
<div style='height:160px'></div></div></div>",
        400.,
        200.,
    );
    assert!(is(top(&found, 25., 10.), 0x0000ff), "first header");
    assert!(is(top(&found, 75., 10.), 0x0000ff), "repeated header");
}

#[test]
fn statically_positioned_box_is_routed_from_its_laid_out_place_and_paints_above_later_flow() {
    // The source lays the box out where its layout put it, while the plan used
    // its resolved static position; it still paints above a later in-flow sibling.
    let found = quads(
        "<style>body{margin:0}</style>\
<div style='columns:2;gap:0;width:100px;height:100px;column-fill:auto'>\
<div style='position:relative'><div style='height:100px'></div>\
<div><div style='position:absolute;width:50px;height:100px;background:#0000ff'></div>\
<div style='height:100px;background:#ff0000'></div></div></div></div>",
        400.,
        200.,
    );
    assert!(is(top(&found, 75., 50.), 0x0000ff), "{found:?}");
}

mod held;
mod hits;
mod pages;

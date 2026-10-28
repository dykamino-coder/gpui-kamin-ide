//! Продолжения на листах: свой лист, слои fixed, имена страниц, поздняя статическая
//! позиция корня.

use super::super::printed;
use super::{any, opaque};

#[test]
fn fixed_layer_inside_a_continued_root_keeps_the_pages_painting() {
    // The fixed box paints in its own layer, outside its root's capture: its
    // decoration layer must not be admitted into that capture's placements.
    let found = printed(
        "<style>body{margin:0}</style>\
<div style='position:absolute;height:300vh;width:100px'>\
<div style='position:fixed;bottom:0;width:40px;height:20px;\
background:linear-gradient(red, blue)'></div></div>",
        800.,
        600.,
    );
    let sheets = found.iter().filter(|(_, c)| opaque(*c) == 0xffffff).count();
    assert!(sheets >= 3, "three sheets painted: {found:?}");
}

#[test]
fn root_painted_by_two_native_sources_keeps_both_records() {
    // The fixed box repeats per sheet through its own source, which also records
    // its root: the slot replays both records instead of the last one only.
    let found = printed(
        "<style>body{margin:0}</style>\
<div style='position:absolute;height:300vh;width:100px'>\
<div style='height:20px;background:#0000ff'></div>\
<div style='position:fixed;bottom:0;width:40px;height:20px;background:#00ff00'></div></div>",
        800.,
        600.,
    );
    assert!(any(&found, 0x0000ff), "first sheet content");
}

#[test]
fn named_pages_inside_an_absolute_box_do_not_break_it() {
    let found = printed(
        "<style>body{margin:0}</style>\
<div style='position:absolute;left:0;top:0;width:50px'>\
<div style='page:a;height:20px;background:#0000ff'></div>\
<div style='page:b;height:20px;background:#00ff00'></div></div>",
        800.,
        600.,
    );
    // The second box stays under the first instead of continuing on a new sheet.
    let second: Vec<_> = found
        .iter()
        .filter(|(_, c)| opaque(*c) == 0x00ff00)
        .collect();
    assert_eq!(second.len(), 1, "{found:?}");
    assert_eq!(f32::from(second[0].0.origin.y), 68.);
}

#[test]
fn box_nested_in_a_continued_box_is_placed_in_the_same_continuous_flow() {
    // 100vh + 300vh lands on the fifth sheet, not past it.
    let found = printed(
        "<style>body{margin:0}</style><div style='height:300vh'></div>\
<div style='position:absolute;top:100vh;width:50px'>\
<div style='position:absolute;top:300vh;width:50px;height:10px;background:#0000ff'></div></div>",
        800.,
        600.,
    );
    let sheets = found.iter().filter(|(_, c)| opaque(*c) == 0xffffff).count();
    assert_eq!(sheets, 5, "{found:?}");
    assert!(any(&found, 0x0000ff));
}

#[test]
fn sheets_past_the_flow_come_from_the_native_placement_of_root_boxes() {
    // No in-flow content: the sheets exist only because the root absolute boxes
    // are placed there (css-position-3 §abspos-breaking), not from a reach estimate.
    for (style, pages) in [
        ("top:300vh;height:10px", 4),
        ("bottom:-200vh;height:10px", 3),
    ] {
        let found = printed(
            &format!(
                "<style>body{{margin:0}}</style>\
<div style='position:absolute;width:50px;{style};background:#0000ff'></div>"
            ),
            800.,
            600.,
        );
        let sheets = found.iter().filter(|(_, c)| opaque(*c) == 0xffffff).count();
        assert_eq!(sheets, pages, "{style}: {found:?}");
        assert!(any(&found, 0x0000ff), "{style}: the box is painted");
    }
}

#[test]
fn overflow_past_a_short_box_reserves_sheets_from_its_measured_flow() {
    // A short box whose content overflows it: the sheets after it come from
    // the placed absolute continuation and the measured in-flow overflow.
    for inner in [
        "position:absolute;top:0;width:50px;height:250vh",
        "width:50px;height:250vh",
    ] {
        let found = printed(
            &format!(
                "<style>body{{margin:0}}</style><div style='position:relative;height:20px'>\
<div style='{inner};background:#0000ff'></div></div>"
            ),
            800.,
            600.,
        );
        let sheets = found.iter().filter(|(_, c)| opaque(*c) == 0xffffff).count();
        assert_eq!(sheets, 3, "{inner}: {found:?}");
        let blue = found.iter().filter(|(_, c)| opaque(*c) == 0x0000ff).count();
        assert_eq!(blue, 3, "{inner}: one fragment per sheet");
    }
}

#[test]
fn anchor_positioned_box_on_a_sheet_paints_beside_its_anchor() {
    let found = printed(
        "<style>body{margin:0}</style>\
<div style='width:100px;height:100px'><div style='anchor-name:--a;width:50px;height:100px;background:#00ff00'></div></div>\
<div style='position:absolute;position-anchor:--a;top:anchor(top);left:anchor(right);width:50px;height:100px;background:#0000ff'></div>",
        800.,
        600.,
    );
    let blue: Vec<_> = found
        .iter()
        .filter(|(_, c)| opaque(*c) == 0x0000ff)
        .collect();
    let green: Vec<_> = found
        .iter()
        .filter(|(_, c)| opaque(*c) == 0x00ff00)
        .collect();
    assert_eq!(blue.len(), 1, "{found:?}");
    assert_eq!(blue[0].0.origin.y, green[0].0.origin.y);
    assert_eq!(blue[0].0.origin.x, green[0].0.right());
}

#[test]
fn late_placed_root_carries_its_admission_to_its_static_position() {
    // The box's layout puts it after its line; its static position, applied at
    // prepaint, puts it at the top of its parent. Its admission follows it.
    let html = "<div style='page:a;height:20px'></div><div style='page:b'><div style='page:c'>\
<div style='position:absolute;left:100px'><span style='display:inline-block;width:10px;height:10px;background:#0000ff'></span></div>\
<div style='page:a;height:20px'></div></div></div>";
    let found = printed(html, 800., 600.);
    let blue: Vec<_> = found
        .iter()
        .filter(|(_, c)| opaque(*c) == 0x0000ff)
        .collect();
    assert_eq!(blue.len(), 1, "{found:?}");
    // Its line in the second block, under the 20px block and the 48px margin
    // (where the legacy copy painted it); unshifted it landed a line higher.
    assert_eq!(f32::from(blue[0].0.origin.y), 81.5);
}

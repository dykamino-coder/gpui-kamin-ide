//! Atomic inline sizing: shrink-to-fit only for widths that depend on the block
//! (CSS 2.1 §10.3.9), and percentages resolved against the block only when
//! present (CSS 2.1 §10.5, §8.4).

use super::quads;

fn size_of(html: &str, colour: u32) -> (f32, f32) {
    quads(html, 300., 200.)
        .into_iter()
        .find(|(_, c)| c.to_rgb() == gpui::rgb(colour))
        .map(|(b, _)| (f32::from(b.size.width), f32::from(b.size.height)))
        .expect("painted atom")
}

#[test]
fn auto_width_atom_still_shrinks_to_the_containing_block() {
    // min-content 40px (one cell), max-content 80px, available 50px.
    let html = "<style>body{margin:0} i{display:inline-block;width:40px;height:10px}</style>\
                <div style='width:50px'><span style='display:inline-block;background:red'>\
                <i></i><i></i></span></div>";
    assert_eq!(size_of(html, 0xff0000).0, 50.0);
}

#[test]
fn fixed_width_atom_keeps_its_width_in_a_narrower_block() {
    let html = "<style>body{margin:0}</style><div style='width:20px'>\
                <span style='display:inline-block;width:30px;height:10px;background:blue'></span>\
                </div>";
    assert_eq!(size_of(html, 0x0000ff).0, 30.0);
}

#[test]
fn percentages_of_an_atom_resolve_against_its_block() {
    let html = "<style>body{margin:0}</style><div style='width:200px;height:100px'>\
                <span style='display:inline-block;width:10px;height:50%;\
                padding-left:10%;background:red'></span></div>";
    assert_eq!(size_of(html, 0xff0000), (30.0, 50.0));
}

#[test]
fn an_atom_without_percentages_keeps_its_own_sizes() {
    let html = "<style>body{margin:0}</style><div style='width:200px;height:100px'>\
                <span style='display:inline-block;width:10px;height:20px;\
                padding-left:4px;background:blue'></span></div>";
    assert_eq!(size_of(html, 0x0000ff), (14.0, 20.0));
}

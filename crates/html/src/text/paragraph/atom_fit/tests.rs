//! Shrink-to-fit atoms (CSS 2.1 §10.3.9): only atoms whose width depends on
//! the containing block are refitted; fixed-width atoms keep their width.

use crate::paint::contract_tests::rendered::quads;

fn width_of(html: &str, colour: u32) -> f32 {
    quads(html, 200., 100.)
        .into_iter()
        .find(|(_, c)| c.to_rgb() == gpui::rgb(colour))
        .map(|(b, _)| f32::from(b.size.width))
        .expect("painted atom")
}

#[test]
fn auto_width_atom_still_shrinks_to_the_containing_block() {
    // min-content 40px (one cell), max-content 80px, available 50px.
    let html = "<style>body{margin:0} i{display:inline-block;width:40px;height:10px}</style>\
                <div style='width:50px'><span style='display:inline-block;background:red'>\
                <i></i><i></i></span></div>";
    assert_eq!(width_of(html, 0xff0000), 50.0);
}

#[test]
fn fixed_width_atom_keeps_its_width_in_a_narrower_block() {
    let html = "<style>body{margin:0}</style><div style='width:20px'>\
                <span style='display:inline-block;width:30px;height:10px;background:blue'></span>\
                </div>";
    assert_eq!(width_of(html, 0x0000ff), 30.0);
}

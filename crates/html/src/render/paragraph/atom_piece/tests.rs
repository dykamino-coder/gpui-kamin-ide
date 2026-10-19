//! Inline containing-block offers survive static-position routing.

use super::*;
use crate::style::computed::Position;

#[test]
fn a_static_absolute_atom_uses_the_inline_containing_block_route() {
    let mut nodes = crate::dom::parse("<span></span>", "");
    let crate::dom::Node::Element(mut element) = nodes.remove(0) else {
        panic!("element")
    };
    element.style.position = Some(Position::Absolute);
    element.style.inline_display = Some(true);
    let opts = RenderOpts {
        text: gpui::TextStyle::default(),
        viewport: (800.0, 600.0),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    for offered in [false, true] {
        let piece = atom_piece_in(
            &element,
            &Computed::default(),
            &opts,
            false,
            offered,
            &AtomRuby::for_atom(&element),
        )
        .expect("absolute inline");
        assert_eq!(
            matches!(piece, inline::Piece::Overlay(..)),
            !offered,
            "offered={offered}"
        );
    }
}

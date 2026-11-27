//! Тесты: маршрутизация статической позиции учитывает систему координат абзаца.

use super::*;
use crate::style::computed::BoxStyle;
use crate::style::computed::Position;

#[test]
fn static_absolute_atoms_use_the_containing_block_route_in_horizontal_frames() {
    let mut nodes = crate::document::parsed::parse("<span></span>", "");
    let crate::dom::Node::Element(mut element) = nodes.remove(0) else {
        panic!("element")
    };
    element.style.used_mut().reset_positioned.position = Some(Position::Absolute);
    element.style.used_mut().reset_context.inline_display = Some(true);
    let opts = RenderOpts {
        text: gpui::TextStyle::default(),
        viewport: (800.0, 600.0),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    for (offered, turned) in [(false, false), (true, false), (false, true), (true, true)] {
        let piece = atom_piece_in(
            &element,
            &{
                let mut flow = BoxStyle::default();
                flow.tree.turned_line = turned;
                flow
            },
            &opts,
            false,
            offered,
            &AtomRuby::for_atom(&element),
        )
        .expect("absolute inline");
        assert_eq!(
            matches!(piece, inline::Piece::Overlay(..)),
            !offered || turned,
            "offered={offered}, turned={turned}"
        );
    }
}

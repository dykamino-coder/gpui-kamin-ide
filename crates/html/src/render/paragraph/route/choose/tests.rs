//! Tests of the paragraph route choice (`route_pieces`).

use super::*;
use crate::style::computed::Computed;

#[test]
fn physical_rows_keep_overlay_only_content_out_of_the_turned_paragraph() {
    for physical in [false, true] {
        for text in ["", "text"] {
            let mut opts = RenderOpts {
                text: gpui::TextStyle::default(),
                viewport: (800.0, 600.0),
                normal_line_height: 1.2,
                cx: Default::default(),
                frag: Default::default(),
            };
            opts.cx.inline.pure_atom_paragraph = physical;
            let inherited = Computed {
                reset_context: crate::style::computed::StyleGroup::new(
                    crate::style::computed::ResetContext {
                        no_select: Some(true),
                        ..Default::default()
                    },
                ),
                ..Default::default()
            };
            let pieces = vec![
                inline::Piece::Text {
                    text: text.into(),
                    style: BoxStyle::from(inherited.clone()),
                },
                inline::Piece::Overlay(div().into_any_element(), Default::default()),
            ];
            let mut element = route_pieces(
                pieces,
                &BoxStyle::from(inherited.clone()),
                &opts,
                None,
                Default::default(),
                None,
                None,
                vec![],
                None,
                (None, None),
            );
            assert_eq!(
                element.downcast_mut::<gpui::StyledText>().is_some(),
                !physical || !text.is_empty(),
                "physical={physical}, text={text:?}",
            );
        }
    }
}

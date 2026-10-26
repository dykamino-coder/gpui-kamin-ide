//! Контракт paint: прежняя нормализация не меняет опубликованный стиль.

use super::*;
use crate::style::computed::BoxStyle;
use crate::style::css::parse_decls;

#[test]
fn legacy_block_alignment_matches_normalized_input_without_style_writes() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    for display in ["flex", "grid"] {
        for mode in ["horizontal-tb", "vertical-rl"] {
            let mut input = Computed::default();
            input.apply_decls(
                &parse_decls(&format!(
                    "display:{display};writing-mode:{mode};align-items:stretch;justify-content:center"
                )),
                doc.values(),
            );
            input.webkit_box = Some(true);
            input.webkit_box_vertical = Some(true);
            input.line_clamp = Some(2);
            let mut normalized = input.clone();
            normalized.align_items = None;
            normalized.justify_content = None;
            let input = BoxStyle::from(input);
            let mut actual = apply_layout(gpui::div(), &input, doc.values());
            let mut expected = apply_layout(gpui::div(), &BoxStyle::from(normalized), doc.values());
            assert_eq!(actual.style().align_items, expected.style().align_items);
            assert_eq!(
                actual.style().justify_content,
                expected.style().justify_content
            );
            assert_eq!(input.align_items, Some(Align::Stretch));
            assert_eq!(input.justify_content, Some(Justify::Center));
        }
    }
}

#[test]
fn transferred_constraints_reach_native_style_without_replacing_css_inputs() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let input = BoxStyle::from(Computed {
        aspect_ratio: Some(2.0),
        min_height: Some(Len::Px(20.0)),
        max_width: Some(Len::Px(200.0)),
        ..Computed::default()
    });
    let mut actual = apply_layout(gpui::div(), &input, doc.values());
    assert_eq!(actual.style().min_size.width, Some(px(40.0).into()));
    assert_eq!(actual.style().max_size.height, Some(px(100.0).into()));
    assert_eq!(input.min_width, None);
    assert_eq!(input.max_height, None);
}

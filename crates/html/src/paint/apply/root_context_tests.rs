//! CSS 2.1 section 9.4.1: the root establishes a formatting context.

use super::*;

#[test]
fn root_block_establishes_a_formatting_context_without_changing_flex_or_grid() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    for (css, expected) in [
        ("", gpui::Display::FlowRoot),
        ("display:block", gpui::Display::FlowRoot),
        ("display:list-item", gpui::Display::FlowRoot),
        ("display:flex", gpui::Display::Flex),
        ("display:grid", gpui::Display::Grid),
    ] {
        let mut c = Computed::default();
        c.apply_decls(&parse_decls(css), vcx);
        c.reset_context.root_box = true;
        let mut d = apply(gpui::div(), &BoxStyle::from(c), vcx);
        assert_eq!(d.style().display, Some(expected), "{css}");
    }
    assert_ne!(
        styled("display:block", vcx).display,
        Some(gpui::Display::FlowRoot)
    );
}

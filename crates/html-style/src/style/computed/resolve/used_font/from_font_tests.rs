//! A cascaded `font-size-adjust: from-font` is a comparable value.

use super::tests::{doc, styled};

#[test]
fn a_cascaded_from_font_equals_itself() {
    let doc = doc();
    let vcx = doc.values();
    let css = "font-family:narrow;font-size-adjust:cap-height from-font";
    let (a, b) = (styled(css, vcx), styled(css, vcx));
    // Two cascades of one declaration are one value: a restyle compares
    // them (`Computed::damage`) and a memo keys by the font group.
    assert!(a.inh.font == b.inh.font);
    assert_eq!(
        a.inh.font.font_size_adjust,
        Some((1 + crate::style::computed::SIZE_ADJUST_FROM_FONT, 0.0))
    );
    assert_eq!(a.font_adjustment(), None);
}

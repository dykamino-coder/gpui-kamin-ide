//! CSS Scroll Behavior is non-inherited and participates in the CSS-wide cascade.
use super::*;
use crate::style::computed::BoxStyle;

#[test]
fn behavior_uses_the_regular_cascade_and_explicit_inheritance() {
    let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut parent = Computed::default();
    parent.apply_one("scroll-behavior", "smooth", vcx);
    parent.apply_one("scroll-behavior", "invalid", vcx);
    assert_eq!(parent.reset_box_layout.scroll.smooth, Some(true));
    let inherit = |own: &Computed| {
        crate::style::cascade::inherit::inherit(&BoxStyle::from(parent.clone()), own, vcx)
    };
    assert_eq!(
        inherit(&Computed::default()).reset_box_layout.scroll.smooth,
        None
    );
    let mut own = Computed::default();
    own.apply_one("scroll-behavior", "inherit", vcx);
    assert_eq!(inherit(&own).reset_box_layout.scroll.smooth, Some(true));
    for reset in ["auto", "initial", "unset", "revert", "revert-layer"] {
        own.apply_one("scroll-behavior", "smooth", vcx);
        own.apply_one("scroll-behavior", reset, vcx);
        assert_eq!(
            inherit(&own).reset_box_layout.scroll.smooth,
            Some(false),
            "{reset}"
        );
    }
}

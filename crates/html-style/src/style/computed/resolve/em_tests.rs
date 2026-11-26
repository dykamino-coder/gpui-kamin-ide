//! Tests: font-relative lengths deferred to the font size (shadows, transforms) resolve `rem` against the root.

use crate::style::computed::{BoxStyle, Computed};

#[test]
fn deferred_shadow_lengths_resolve_rem_against_the_root_font() {
    let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
    crate::style::values::value::set_root_metrics(&doc, 1.0, 1.2);
    let vcx = doc.values();
    let mut own = Computed::default();
    own.apply_one("font-size", "20px", vcx);
    own.apply_one("box-shadow", "0 0 3rem 2em #fff", vcx);
    let computed = crate::style::cascade::inherit::inherit(&BoxStyle::default(), &own, vcx);
    let shadow = &computed.reset_effects.shadows[0];
    assert_eq!((shadow.blur, shadow.spread), (3.0, 40.0));
}

//! Verify isolated mask resolution uses the canonical computed image units.

use crate::style::computed::Computed;
use crate::style::values::value::Len;

#[test]
fn a_single_mask_matches_the_full_image_resolver() {
    let style = Computed {
        font_size: Some(Len::Px(20.0)),
        line_height: Some(Len::Pct(1.5)),
        bg_image: Some("linear-gradient(red, blue)".into()),
        ..Computed::default()
    };
    for raw in [
        "linear-gradient(red 1em, blue 2lh)",
        "conic-gradient(at 1ch 2ex, red, blue)",
        "linear-gradient(red 10px, blue 20px)",
        "url(mask.svg)",
    ] {
        let mut full = style.clone();
        full.bg_image = None;
        full.gradient_raw = None;
        full.mask_image = Some(raw.into());
        full.resolve_image_lengths();
        assert_eq!(Some(style.resolve_mask_image(raw.into())), full.mask_image);
    }
    assert_eq!(
        style.bg_image.as_deref(),
        Some("linear-gradient(red, blue)")
    );
    assert!(style.mask_image.is_none());
}

#[test]
fn absolute_masks_preserve_the_owned_input() {
    let style = Computed::default();
    let raw = String::from("linear-gradient(red 0px, blue 10px)");
    let pointer = raw.as_ptr();
    let resolved = style.resolve_mask_image(raw);
    assert_eq!(resolved.as_ptr(), pointer);
}

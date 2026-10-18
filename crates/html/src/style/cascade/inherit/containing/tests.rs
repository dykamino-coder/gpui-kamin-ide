//! Containing-block inheritance and transformed fixed-background tests.

use crate::style::computed::Computed;
use crate::style::values::value::Len;

#[test]
fn fixed_background_scrolls_under_a_transform() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let parent = Computed {
        translate: Some((Len::Px(1.0), Len::Px(0.0))),
        ..Default::default()
    };
    let own = Computed {
        bg_fixed: Some(true),
        ..Default::default()
    };
    assert_eq!(
        super::super::inherit(&parent, &own, vcx).bg_fixed,
        Some(false)
    );
    let plain = super::super::inherit(&Computed::default(), &own, vcx);
    assert_eq!(plain.bg_fixed, Some(true));
}

//! Containing-block inheritance and transformed fixed-background tests.

use crate::style::computed::Computed;
use crate::style::values::value::Len;

#[test]
fn fixed_background_scrolls_under_a_transform() {
    let parent = Computed {
        translate: Some((Len::Px(1.0), Len::Px(0.0))),
        ..Default::default()
    };
    let own = Computed {
        bg_fixed: Some(true),
        ..Default::default()
    };
    assert_eq!(super::super::inherit(&parent, &own).bg_fixed, Some(false));
    let plain = super::super::inherit(&Computed::default(), &own);
    assert_eq!(plain.bg_fixed, Some(true));
}

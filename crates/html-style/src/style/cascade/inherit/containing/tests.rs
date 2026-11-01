//! Тесты наследования содержащего блока и fixed-фона у трансформированного предка.

use crate::style::computed::{BoxStyle, Computed};
use crate::style::values::value::Len;

#[test]
fn fixed_background_scrolls_under_a_transform() {
    let doc = crate::values::DocumentValues::detached(crate::style::css::Media::default());
    let vcx = doc.values();
    let parent = Computed {
        reset_transforms: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetTransforms {
                translate: Some((Len::Px(1.0), Len::Px(0.0))),
                ..Default::default()
            },
        ),
        ..Default::default()
    };
    let own = Computed {
        reset_background: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetBackground {
                bg_fixed: Some(true),
                ..Default::default()
            },
        ),
        ..Default::default()
    };
    assert_eq!(
        super::super::inherit(&BoxStyle::from(parent), &own, vcx)
            .reset_background
            .bg_fixed,
        Some(false)
    );
    let plain = super::super::inherit(&BoxStyle::default(), &own, vcx);
    assert_eq!(plain.reset_background.bg_fixed, Some(true));
}

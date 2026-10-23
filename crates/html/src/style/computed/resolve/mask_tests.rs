//! Тесты: источники маски делят вычисленное разрешение изображений и сохраняют порядок слоёв.

use crate::style::computed::{Computed, ImageSource};
use crate::style::values::value::Len;

#[test]
fn shape_image_uses_computed_lengths_and_clears_with_none_or_basic_shape() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut own = Computed::default();
    own.apply_one("font-size", "20px", vcx);
    own.apply_one("line-height", "2", vcx);
    own.apply_one(
        "shape-outside",
        "linear-gradient(currentcolor 1em, transparent 2lh)",
        vcx,
    );
    let computed = crate::style::cascade::inherit::inherit(&Computed::default(), &own, vcx);
    let (gradient, _) = computed.shape_image.as_ref().unwrap().gradient().unwrap();
    assert_eq!(gradient.stops_px[0].1, 20.0);
    assert_eq!(gradient.stops_px[1].1, 80.0);
    own.apply_one("shape-outside", "circle(50%)", vcx);
    assert!(own.shape_image.is_none());
    own.apply_one("shape-outside", "url(shape.png)", vcx);
    assert_eq!(own.shape_image, Some(ImageSource::Url("shape.png".into())));
    own.apply_one("shape-outside", "none", vcx);
    assert!(own.shape_image.is_none());
    assert!(own.shape_outside.is_none());
}

#[test]
fn masks_and_backgrounds_share_computed_font_lengths() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    for raw in [
        "linear-gradient(red 1em, blue 2lh)",
        "conic-gradient(at 1ch 2ex, red, blue)",
        "linear-gradient(red 10px, blue 20px)",
        "url(mask.svg)",
    ] {
        let mut style = {
            let mut with_inherited = Computed {
                ..Computed::default()
            };
            with_inherited.font_size = Some(Len::Px(20.0));
            with_inherited.line_height = Some(Len::Pct(1.5));
            with_inherited
        };
        style.apply_one("mask-image", raw, vcx);
        style.bg_image = ImageSource::parse(raw, vcx);
        style.resolve_em(20.0, vcx);
        assert_eq!(
            style.bg_image.as_ref(),
            style.mask_image.as_ref().unwrap().first()
        );
    }
}

#[test]
fn mask_none_clears_the_previous_image() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut style = Computed::default();
    style.apply_one("mask-image", "url(mask.svg)", vcx);
    assert!(style.mask_image.is_some());
    style.apply_one("mask-image", "none", vcx);
    assert!(style.mask_image.is_none());
}

#[test]
fn absent_layers_keep_their_composition_indices() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let mut style = Computed::default();
    style.apply_one("mask-image", "url(a.svg), none, url(b.svg)", doc.values());
    assert_eq!(
        style.mask_image.unwrap(),
        vec![
            ImageSource::Url("a.svg".into()),
            ImageSource::Invalid,
            ImageSource::Url("b.svg".into())
        ]
    );
}

#[test]
fn inherited_mask_uses_own_line_height_before_group_construction() {
    use crate::style::cascade::inherit::inherit;
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut parent = Computed::default();
    parent.apply_one("font-size", "50px", vcx);
    parent.apply_one("line-height", "2", vcx);
    let mut own = Computed::default();
    own.apply_one("mask-image", "conic-gradient(at 1lh 50px, red, blue)", vcx);
    let computed = inherit(&parent, &own, vcx);
    let ImageSource::Conic(conic) = &computed.mask_image.unwrap()[0] else {
        panic!("computed conic mask");
    };
    assert_eq!(conic.position.x, Some(Len::Px(100.0)));
    assert_eq!(conic.position.y, Some(Len::Px(50.0)));
    // The declaration remains reusable when another parent supplies the font.
    assert!(own.mask_image.unwrap()[0].needs_line_height(vcx));
}

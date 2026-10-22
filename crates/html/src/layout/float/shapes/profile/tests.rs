//! Проверки альфа-профиля и его физических осей.

use super::*;

#[test]
fn image_alpha_profile_consumes_typed_source_without_css_replay() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut own = crate::style::computed::Computed::default();
    own.apply_one(
        "shape-outside",
        "linear-gradient(to right, black 50%, transparent 50%)",
        vcx,
    );
    let style = crate::style::cascade::inherit::inherit(&Default::default(), &own, vcx);
    let b = ShapeBox {
        mw: 8.0,
        mh: 2.0,
        rx: 0.0,
        ry: 0.0,
        rw: 8.0,
        rh: 2.0,
        cx: 0.0,
        cy: 0.0,
        cw: 8.0,
        ch: 2.0,
        radius: [(0.0, 0.0); 4],
        threshold: 0.0,
    };
    let profile = shape_profile(("", style.shape_image.as_ref()), &b, 0.0, -1, vcx).unwrap();
    assert_eq!(profile, vec![4.0, 4.0]);
}

#[test]
fn threshold_is_strict_and_vertical_profile_keeps_physical_sampling() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let image =
        ImageSource::parse("linear-gradient(to right, black 50%, transparent 50%)", vcx).unwrap();
    let mut b = ShapeBox {
        mw: 8.0,
        mh: 2.0,
        rx: 0.0,
        ry: 0.0,
        rw: 8.0,
        rh: 2.0,
        cx: 0.0,
        cy: 0.0,
        cw: 8.0,
        ch: 2.0,
        radius: [(0.0, 0.0); 4],
        threshold: 0.0,
    };
    assert_eq!(
        shape_profile_block(("", Some(&image)), &b, 0.0, -1, vcx).unwrap(),
        vec![0.0, 0.0, 0.0, 0.0, 2.0, 2.0, 2.0, 2.0]
    );
    b.threshold = 1.0;
    assert_eq!(
        shape_profile(("", Some(&image)), &b, 0.0, -1, vcx).unwrap(),
        vec![0.0; 2]
    );
    assert_eq!(
        mask_intervals(&[127, 128, 255], 3, 1, 128.0 / 255.0),
        vec![Some((2, 3))]
    );
}

#[test]
fn polygon_sample_cells_preserve_mirrored_extents() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let b = ShapeBox {
        mw: 40.0,
        mh: 40.0,
        rx: 0.0,
        ry: 0.0,
        rw: 40.0,
        rh: 40.0,
        cx: 0.0,
        cy: 0.0,
        cw: 40.0,
        ch: 40.0,
        radius: [(0.0, 0.0); 4],
        threshold: 0.0,
    };
    let raw = "polygon(50% 0%, 100% 50%, 50% 100%, 0% 50%)";
    let a = shape_profile((raw, None), &b, 0.0, -1, vcx).unwrap();
    let b = shape_profile((raw, None), &b, 0.0, 1, vcx).unwrap();
    assert_eq!(a, b);
}

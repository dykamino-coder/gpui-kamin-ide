//! Primitive scale interpolation and singular matrix fallback.

use super::*;

#[test]
fn singular_scale_interpolates_argument_by_argument() {
    let mut zero = Transform::default();
    zero.push(Transform::diag(0.0, 0.0), NO_SHIFT);
    zero.scale_primitive = true;
    let one = Transform::default();
    let mid = zero.lerp_2d(&one, 0.25).unwrap();
    assert_eq!(mid.lin, [[0.25, 0.0], [0.0, 0.25]]);
    assert_eq!(
        one.lerp_2d(&zero, 1.0).unwrap().lin,
        [[0.0, 0.0], [0.0, 0.0]]
    );
    // A rotated singular side is not a pair of scales: discrete.
    let mut skewed = Transform::default();
    skewed.push([[0.0, 1.0], [0.0, 0.0]], NO_SHIFT);
    assert!(skewed.lerp_2d(&one, 0.5).is_none());
}

#[test]
fn singular_matrix_and_composed_scales_do_not_use_primitive_interpolation() {
    let values = crate::values::DocumentValues::detached(crate::style::css::Media::default());
    let parse = |raw| {
        let mut style = crate::style::computed::Computed::default();
        style.apply_one("transform", raw, values.values());
        style.reset_transforms.transform.unwrap()
    };
    let identity = Transform::default();
    for raw in ["scale(0)", "scaleX(0)", "scaleY(0)"] {
        assert!(parse(raw).lerp_2d(&identity, 0.25).is_some(), "{raw}");
    }
    for raw in [
        "matrix(0,0,0,0,0,0)",
        "scale(0) translateX(10px)",
        "scale(0) scale(2)",
    ] {
        assert!(parse(raw).lerp_2d(&identity, 0.25).is_none(), "{raw}");
    }
}

#[test]
fn matching_scale_primitives_keep_negative_axes_and_zero_crossings() {
    let values = crate::values::DocumentValues::detached(crate::style::css::Media::default());
    let parse = |raw| {
        let mut style = crate::style::computed::Computed::default();
        style.apply_one("transform", raw, values.values());
        style.reset_transforms.transform.unwrap()
    };
    let a = parse("scale(-1, 2)");
    let b = parse("scale(1, -2)");
    assert_eq!(a.lerp_2d(&b, 0.25).unwrap().lin, [[-0.5, 0.0], [0.0, 1.0]]);
    assert_eq!(a.lerp_2d(&b, 0.5).unwrap().lin, [[0.0, 0.0], [0.0, 0.0]]);
}

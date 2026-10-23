//! Валидные значения проходят проверку грамматики и доходят до вычисленного стиля.

use crate::style::computed::{Computed, ParseResult};
use crate::style::css::LonghandId;

fn accepted(key: &str, value: &str) -> bool {
    let st = crate::document::state::DocState::new(crate::style::css::Media::default());
    Computed::declaration_result(key, value, st.values()) == ParseResult::Accepted
}

fn check(key: &str, valid: &[&str], invalid: &[&str]) {
    for v in valid {
        assert!(accepted(key, v), "valid {key}: {v} was rejected");
    }
    for v in invalid {
        assert!(!accepted(key, v), "invalid {key}: {v} was accepted");
    }
}

#[test]
fn transform_functions_accept_every_angle_unit_and_range() {
    check(
        "transform",
        &[
            "rotate(180deg)",
            "rotate(45deg) rotate(400grad)",
            "rotate(45deg) rotate(1turn)",
            "rotate(-135deg) rotate(3.1415926535897932384626433rad)",
            "skew(45deg,135deg)",
            "rotateY(180deg)",
            "rotatex(180deg) scaleZ(-1)",
            "translateX(200px) rotate(180deg) translateY(-100%)",
            "rotate(0)",
            "scale3d(50%, 1, 2)",
            "translate3d(10px, 50%, 2px)",
        ],
        &[
            "rotate(45)",
            "rotate(10px)",
            "rotate(50%)",
            "translateZ(50%)",
            "scale(1px)",
        ],
    );
}

#[test]
fn rotated_transform_reaches_the_matrix() {
    let st = crate::document::state::DocState::new(crate::style::css::Media::default());
    let mut c = Computed::default();
    c.apply_one("transform", "rotate(180deg)", st.values());
    let t = c.transform.expect("rotate(180deg) applied");
    assert!((t.rotate_rad - std::f32::consts::PI).abs() < 1e-4);
}

#[test]
fn individual_rotate_and_offset_rotate_accept_full_angles() {
    check(
        "rotate",
        &[
            "180deg",
            "-180deg",
            "0.5turn",
            "z 45deg",
            "45deg x",
            "1 0 0 90deg",
            "none",
        ],
        &["45", "0", "x y 45deg", "1 0 90deg", "45deg 10deg"],
    );
    check(
        "offset-rotate",
        &["auto", "reverse", "270deg", "auto 180deg", "-90deg reverse"],
        &["auto auto", "10deg 20deg", "0", "auto 10px"],
    );
    let st = crate::document::state::DocState::new(crate::style::css::Media::default());
    let mut c = Computed::default();
    c.apply_one("rotate", "180deg", st.values());
    assert!(
        c.rotate_prop
            .is_some_and(|r| (r - std::f32::consts::PI).abs() < 1e-4)
    );
}

#[test]
fn shape_box_may_precede_the_shape_function() {
    {
        check(
            "shape-outside",
            &[
                "border-box circle(40%)",
                "content-box ellipse(50% 25% at 50% 50%)",
                "padding-box circle(farthest-side at -10px)",
                "circle(40%) margin-box",
            ],
            &[
                "circle(40%) circle(10%)",
                "border-box content-box",
                "garbage(1)",
            ],
        );
    }
}

#[test]
fn line_clamp_accepts_strings_auto_and_ellipsis_keywords() {
    check(
        "line-clamp",
        &[
            "none",
            "2 \" CUSTOM\"",
            "2 \"\u{2026} (continued)\"",
            "auto",
            "8 auto",
            "7 no-ellipsis",
            "auto 11",
            "auto no-ellipsis",
            "3 ellipsis auto",
            "ellipsis",
            "1 -webkit-legacy",
            "auto -webkit-legacy",
        ],
        &[
            "0",
            "none 2",
            "3 none",
            "none no-ellipsis",
            "-webkit-legacy",
            "auto auto",
            "2 3",
            "ellipsis no-ellipsis",
        ],
    );
}

#[test]
fn text_emphasis_accepts_currentcolor() {
    check(
        "text-emphasis",
        &["filled dot currentColor", "filled dot transparent"],
        &["filled dot garbage"],
    );
}

#[test]
fn place_shorthands_forward_inherit() {
    let st = crate::document::state::DocState::new(crate::style::css::Media::default());
    let mut c = Computed::default();
    c.apply_one("place-items", "inherit", st.values());
    c.apply_one("place-content", "inherit", st.values());
    for id in [
        LonghandId::AlignItems,
        LonghandId::JustifyItems,
        LonghandId::AlignContent,
        LonghandId::JustifyContent,
    ] {
        assert!(
            c.inherit_longhands.contains(id),
            "place-*: inherit lost {id:?}"
        );
    }
}

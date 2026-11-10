//! Настоящие layout-коробки проецируются углами, с границами плоских контекстов.

use super::*;

fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 0.002, "{actual} != {expected}");
}

#[gpui::test]
fn geometry_projects_perspective_corners_without_an_affine_approximation(cx: &mut TestAppContext) {
    let elements = snapshot(
        cx,
        "<style>body{margin:0}</style><div id=box style='width:100px;height:80px;transform-origin:0 0;transform:perspective(400px) rotateY(60deg)'></div>",
        300.,
    );
    let b = find(&elements, "box").border_box;
    near(b.x, 0.);
    near(b.y, 0.);
    near(
        b.width,
        50. / (1. + 100. * 60_f64.to_radians().sin() / 400.),
    );
    near(b.height, 80.);
}

#[gpui::test]
fn geometry_parent_perspective_affects_children_only(cx: &mut TestAppContext) {
    let elements = snapshot(
        cx,
        "<style>body{margin:0}</style><div id=parent style='width:200px;height:100px;perspective:500px;perspective-origin:0 0'><div id=child style='width:100px;height:50px;transform:translateZ(100px)'></div></div>",
        300.,
    );
    let parent = find(&elements, "parent").border_box;
    near(parent.width, 200.);
    near(parent.height, 100.);
    let child = find(&elements, "child").border_box;
    near(child.width, 125.);
    near(child.height, 62.5);
}

#[gpui::test]
fn geometry_preserve_3d_and_intermediate_flat_box(cx: &mut TestAppContext) {
    for (middle, x) in [
        (
            "transform-style:preserve-3d",
            100. * 60_f64.to_radians().sin(),
        ),
        ("", 0.),
    ] {
        let elements = snapshot(
            cx,
            &format!(
                "<style>body{{margin:0}}</style><div style='width:200px;height:100px;transform:rotateY(60deg);transform-origin:0 0;transform-style:preserve-3d'><div style='{middle}'><div id=child style='width:100px;height:50px;transform:translateZ(100px)'></div></div></div>"
            ),
            300.,
        );
        let b = find(&elements, "child").border_box;
        near(b.x, x);
        near(b.width, 50.);
        near(b.height, 50.);
    }
}

#[gpui::test]
fn geometry_projective_rects_include_behind_eye_and_zero_w_boxes(cx: &mut TestAppContext) {
    for (z, x, y) in [(200, -100., -80.), (100, 0., 0.)] {
        let elements = snapshot(
            cx,
            &format!(
                "<style>body{{margin:0}}</style><div id=box style='width:100px;height:80px;transform-origin:0 0;transform:perspective(100px) translateZ({z}px)'></div>"
            ),
            300.,
        );
        let b = find(&elements, "box").border_box;
        near(b.x, x);
        near(b.y, y);
        near(b.width, 100.);
        near(b.height, 80.);
    }
}

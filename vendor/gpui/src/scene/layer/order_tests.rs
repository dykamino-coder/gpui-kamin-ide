//! Scene layers: a layer's scene draws its commands in the order the scene
//! without layers draws them, as recorded and as a box's run is recorded
//! again.

use crate::scene::draw_sequence;
use crate::{Background, Bounds, ContentMask, Hsla, Quad, ScaledPixels, Scene, Underline, hsla, point, size};

pub(super) fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<ScaledPixels> {
    Bounds {
        origin: point(ScaledPixels(x), ScaledPixels(y)),
        size: size(ScaledPixels(w), ScaledPixels(h)),
    }
}

fn mask() -> ContentMask<ScaledPixels> {
    ContentMask {
        bounds: rect(0., 0., 1000., 1000.),
    }
}

pub(super) fn quad(b: Bounds<ScaledPixels>, color: Hsla) -> Quad {
    Quad {
        bounds: b,
        content_mask: mask(),
        background: Background::from(color),
        ..Default::default()
    }
}

fn underline(b: Bounds<ScaledPixels>) -> Underline {
    Underline {
        order: 0, pad: 0,
        bounds: b,
        content_mask: mask(),
        color: hsla(0., 0., 0., 1.),
        thickness: ScaledPixels(b.size.height.0),
        wavy: false.into(),
    }
}

pub(super) fn env() -> super::BoxEnv {
    super::BoxEnv {
        bounds: Bounds::default(),
        mask: ContentMask {
            bounds: Bounds::default(),
        },
        transformation: crate::TransformationMatrix::unit(),
        opacity: 1.,
        rem_size: crate::px(16.),
        fill: None,
    }
}

/// One paint step of a test page.
pub(super) enum Step {
    Quad(Bounds<ScaledPixels>, Hsla),
    /// A layer (a line) of `bounds` whose quad or underline `ink` passes them.
    Line(Bounds<ScaledPixels>, Bounds<ScaledPixels>, bool),
    /// A box of `area` painting the quads.
    Box(u32, Bounds<ScaledPixels>, Vec<(Bounds<ScaledPixels>, Hsla)>),
}

/// The steps painted into a scene, as a layer recording (`layered`) or a
/// frame without layers; the layer's box runs.
pub(super) fn paint(steps: &[Step], layered: bool) -> (Scene, Vec<super::BoxRange>) {
    let mut scene = Scene::default();
    let mut boxes = Vec::new();
    for step in steps {
        match step {
            Step::Quad(b, c) => scene.insert_primitive(quad(*b, *c)),
            Step::Line(bounds, ink, as_quad) => {
                scene.push_layer(*bounds);
                if *as_quad {
                    scene.insert_primitive(quad(*ink, hsla(0.3, 1., 0.5, 1.)));
                } else {
                    scene.insert_primitive(underline(*ink));
                }
                scene.pop_layer();
            }
            Step::Box(owner, area, fills) => {
                let open = layered && scene.open_box(*owner, *area, env());
                for (b, c) in fills {
                    scene.insert_primitive(quad(*b, *c));
                }
                if open {
                    boxes.extend(scene.close_box());
                }
            }
        }
    }
    (scene, boxes)
}

pub(super) fn drawn(scene: &mut Scene) -> Vec<(String, Bounds<ScaledPixels>)> {
    scene.finish();
    let mut out = Vec::new();
    draw_sequence(scene, &mut out);
    out
}

/// The layer draws the steps as the frame without layers does.
fn assert_as_without_layers(steps: &[Step]) {
    let (mut layered, _) = paint(steps, true);
    let (mut plain, _) = paint(steps, false);
    assert_eq!(drawn(&mut layered), drawn(&mut plain));
}

pub(super) fn root() -> Step { Step::Quad(rect(0., 0., 100., 100.), hsla(0., 0., 1., 1.)) }

#[test]
fn a_line_ink_in_the_gap_between_a_box_area_and_its_fill_keeps_its_order() {
    // The line's underline passes its bounds (to x = 12) into the box's area
    // (from x = 9.5) but its fill starts at x = 11: the fill meets only the
    // root, one bucket below the line, and draws first.
    assert_as_without_layers(&[
        root(),
        Step::Line(rect(0., 0., 10., 10.), rect(0., 0., 12., 10.), false),
        Step::Box(1, rect(9.5, 0., 20.5, 10.), vec![(rect(11., 0., 19., 10.), hsla(0.6, 1., 0.5, 1.))]),
    ]);
}

#[test]
fn a_line_quad_past_its_bounds_and_a_later_box_fill_of_its_bucket_keep_paint_order() {
    // Equal orders draw quads in paint order: the line's quad, then the fill.
    assert_as_without_layers(&[
        root(),
        Step::Line(rect(0., 0., 10., 10.), rect(0., 0., 12., 10.), true),
        Step::Box(1, rect(11., 0., 19., 10.), vec![(rect(11., 0., 19., 10.), hsla(0.6, 1., 0.5, 1.))]),
    ]);
}

#[test]
fn a_box_with_a_fill_and_a_border_over_it_keeps_the_orders_of_both() {
    assert_as_without_layers(&[
        root(),
        Step::Box(
            1,
            rect(10., 10., 50., 50.),
            vec![(rect(10., 10., 50., 50.), hsla(0.6, 1., 0.5, 1.)), (rect(10., 10., 50., 2.), hsla(0., 1., 0.5, 1.))],
        ),
        Step::Line(rect(10., 12., 20., 10.), rect(10., 12., 45., 10.), false),
        Step::Box(2, rect(30., 12., 20., 10.), vec![(rect(31., 12., 19., 10.), hsla(0.1, 1., 0.5, 1.))]),
    ]);
}


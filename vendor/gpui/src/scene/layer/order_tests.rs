//! Scene layers: a layer's scene draws its commands in the order the scene
//! without layers draws them, as recorded and as a box's run is recorded
//! again.

use crate::scene::draw_sequence;
use crate::{Background, Bounds, ContentMask, Hsla, Quad, ScaledPixels, Scene, Underline, hsla, point, size};

fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<ScaledPixels> {
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

fn quad(b: Bounds<ScaledPixels>, color: Hsla) -> Quad {
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

fn env() -> super::BoxEnv {
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
enum Step {
    Quad(Bounds<ScaledPixels>, Hsla),
    /// A layer (a line) of `bounds` whose quad or underline `ink` passes them.
    Line(Bounds<ScaledPixels>, Bounds<ScaledPixels>, bool),
    /// A box of `area` painting the quads.
    Box(u32, Bounds<ScaledPixels>, Vec<(Bounds<ScaledPixels>, Hsla)>),
}

/// The steps painted into a scene, as a layer recording (`layered`) or a
/// frame without layers; the layer's box runs.
fn paint(steps: &[Step], layered: bool) -> (Scene, Vec<super::BoxRange>) {
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

fn drawn(scene: &mut Scene) -> Vec<(String, Bounds<ScaledPixels>)> {
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

fn root() -> Step { Step::Quad(rect(0., 0., 100., 100.), hsla(0., 0., 1., 1.)) }

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

type Fills = Vec<(Bounds<ScaledPixels>, Hsla)>;

/// Record the steps into a layer, paint box `owner`'s run again as `fills`
/// and, if the layer takes it, compare the layer with the steps painted from
/// scratch with those fills; whether the layer took the run.
fn rerecorded_as_from_scratch(mut steps: Vec<Step>, owner: u32, fills: Fills) -> bool {
    let (scene, boxes) = paint(&steps, true);
    let at = boxes.iter().position(|b| b.owner == owner).expect("the box was recorded");
    let area = boxes[at].area;
    let layer = super::LayerHandle::new(super::SceneLayer::finish(scene, Vec::new(), boxes));
    let mut again = Scene::default();
    assert!(again.open_box(owner, area, env()));
    for (b, c) in &fills {
        again.insert_primitive(quad(*b, *c));
    }
    again.close_box();
    let version = layer.content().version();
    let plan = layer.0.content.borrow().check_box(at, &again);
    let Some(plan) = plan else {
        assert_eq!(layer.content().version(), version, "nothing was replaced");
        return false;
    };
    {
        let mut current = layer.0.content.borrow_mut();
        std::rc::Rc::make_mut(&mut current).replace_box(at, again, Vec::new(), plan);
    }
    for step in &mut steps {
        if let Step::Box(o, _, f) = step
            && *o == owner
        {
            *f = fills.clone();
        }
    }
    let (mut plain, _) = paint(&steps, false);
    let mut out = Vec::new();
    draw_sequence(layer.content().scene(), &mut out);
    assert_eq!(out, drawn(&mut plain));
    true
}

fn fill() -> Hsla { hsla(0.6, 1., 0.5, 1.) }

#[test]
fn a_fill_that_appears_beside_a_line_ink_takes_the_order_from_scratch() {
    let steps = vec![
        root(),
        Step::Line(rect(0., 0., 10., 10.), rect(0., 0., 12., 10.), false),
        Step::Box(1, rect(9.5, 0., 20.5, 10.), Vec::new()),
    ];
    assert!(rerecorded_as_from_scratch(steps, 1, vec![(rect(11., 0., 19., 10.), fill())]));
}

#[test]
fn a_border_over_its_fill_and_a_line_takes_the_orders_from_scratch() {
    let steps = vec![
        root(),
        Step::Box(1, rect(10., 10., 50., 50.), vec![(rect(10., 10., 50., 50.), fill())]),
        Step::Line(rect(70., 10., 10., 10.), rect(58., 10., 22., 10.), true),
    ];
    let border = vec![(rect(10., 10., 50., 50.), fill()), (rect(10., 10., 50., 2.), hsla(0., 1., 0.5, 1.))];
    assert!(rerecorded_as_from_scratch(steps, 1, border));
}

#[test]
fn a_fill_that_would_raise_a_later_box_is_refused() {
    let steps = vec![
        root(),
        Step::Box(1, rect(0., 0., 20., 10.), Vec::new()),
        Step::Box(2, rect(20., 0., 20., 10.), vec![(rect(20., 0., 20., 10.), fill())]),
    ];
    assert!(!rerecorded_as_from_scratch(steps, 1, vec![(rect(0., 0., 30., 10.), fill())]));
}

#[test]
fn a_fill_that_goes_from_under_a_quad_only_it_held_is_refused() {
    let steps = vec![
        root(),
        Step::Box(1, rect(10., 10., 40., 40.), vec![(rect(10., 10., 40., 40.), fill())]),
        Step::Quad(rect(20., 20., 10., 10.), hsla(0.3, 1., 0.5, 0.5)),
    ];
    assert!(!rerecorded_as_from_scratch(steps, 1, Vec::new()));
}

#[test]
fn a_fill_that_goes_from_under_a_quad_another_box_holds_is_taken() {
    let steps = vec![
        root(),
        Step::Box(1, rect(10., 10., 40., 40.), vec![(rect(10., 10., 40., 40.), fill())]),
        Step::Box(2, rect(60., 10., 40., 40.), vec![(rect(60., 10., 40., 40.), fill())]),
        Step::Quad(rect(20., 20., 60., 10.), hsla(0.3, 1., 0.5, 0.5)),
    ];
    assert!(rerecorded_as_from_scratch(steps, 1, Vec::new()));
}

#[test]
fn a_colour_painted_again_over_lines_keeps_every_order() {
    let steps = vec![
        root(),
        Step::Line(rect(0., 0., 30., 10.), rect(0., 0., 32., 10.), true),
        Step::Box(1, rect(31., 0., 20., 10.), vec![(rect(31., 0., 20., 10.), fill()), (rect(31., 0., 20., 1.), fill())]),
        Step::Line(rect(40., 0., 30., 10.), rect(30., 0., 40., 10.), false),
    ];
    let red = vec![(rect(31., 0., 20., 10.), hsla(0., 1., 0.5, 1.)), (rect(31., 0., 20., 1.), fill())];
    assert!(rerecorded_as_from_scratch(steps, 1, red));
}
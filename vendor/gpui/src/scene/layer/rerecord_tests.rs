//! Scene layers: a box's run recorded again takes the orders a paint from
//! scratch gives it, and later commands it reaches move as that paint moves
//! them.

use super::order_tests::{Step, drawn, env, paint, quad, rect, root};
use crate::scene::draw_sequence;
use crate::{Bounds, Hsla, ScaledPixels, Scene, hsla};
type Fills = Vec<(Bounds<ScaledPixels>, Hsla)>;

/// Record the steps into a layer, paint box `owner`'s run again as `fills`
/// and, if the layer takes it, compare the layer with the steps painted from
/// scratch with those fills; whether the layer took the run.
fn rerecorded_as_from_scratch(steps: Vec<Step>, owner: u32, fills: Fills) -> bool {
    each_as_from_scratch(steps, owner, vec![fills])
}

/// `rerecorded_as_from_scratch` of each fills in turn into one layer.
fn each_as_from_scratch(mut steps: Vec<Step>, owner: u32, each: Vec<Fills>) -> bool {
    let (scene, boxes) = paint(&steps, true);
    let at = boxes.iter().position(|b| b.owner == owner).expect("the box was recorded");
    let area = boxes[at].area;
    let layer = super::LayerHandle::new(super::SceneLayer::finish(scene, Vec::new(), boxes));
    each.into_iter().all(|fills| again_as_from_scratch(&layer, &mut steps, at, owner, area, fills))
}

fn again_as_from_scratch(
    layer: &super::LayerHandle,
    steps: &mut [Step],
    at: usize,
    owner: u32,
    area: Bounds<ScaledPixels>,
    fills: Fills,
) -> bool {
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
    for step in steps.iter_mut() {
        if let Step::Box(o, _, f) = step
            && *o == owner
        {
            *f = fills.clone();
        }
    }
    let (mut plain, _) = paint(steps, false);
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
fn a_fill_that_raises_a_later_box_takes_the_orders_from_scratch() {
    let steps = vec![
        root(),
        Step::Box(1, rect(0., 0., 20., 10.), Vec::new()),
        Step::Box(2, rect(20., 0., 20., 10.), vec![(rect(20., 0., 20., 10.), fill())]),
    ];
    assert!(rerecorded_as_from_scratch(steps, 1, vec![(rect(0., 0., 30., 10.), fill())]));
}

#[test]
fn a_fill_that_goes_from_under_a_quad_only_it_held_lowers_it() {
    let steps = vec![
        root(),
        Step::Box(1, rect(10., 10., 40., 40.), vec![(rect(10., 10., 40., 40.), fill())]),
        Step::Quad(rect(20., 20., 10., 10.), hsla(0.3, 1., 0.5, 0.5)),
    ];
    assert!(rerecorded_as_from_scratch(steps, 1, Vec::new()));
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
/// A box whose fill comes and goes under its text and a child with its own
/// text, as the board's edits paint it.
fn parent_with_children(fills: Fills) -> Vec<Step> {
    vec![
        root(),
        Step::Box(1, rect(10., 10., 80., 60.), fills),
        Step::Line(rect(12., 12., 40., 10.), rect(12., 12., 44., 10.), false),
        Step::Box(2, rect(12., 30., 60., 30.), vec![(rect(12., 30., 60., 30.), hsla(0.1, 1., 0.5, 1.))]),
        Step::Line(rect(14., 32., 30., 10.), rect(14., 32., 34., 10.), true),
    ]
}

#[test]
fn a_fill_that_appears_and_goes_under_children_moves_them_as_from_scratch() {
    let fill_of_one = vec![(rect(10., 10., 80., 60.), fill())];
    assert!(rerecorded_as_from_scratch(parent_with_children(Vec::new()), 1, fill_of_one.clone()));
    assert!(rerecorded_as_from_scratch(parent_with_children(fill_of_one), 1, Vec::new()));
}
#[test]
fn a_fill_that_comes_goes_and_comes_again_keeps_the_layer_as_from_scratch() {
    let one = vec![(rect(10., 10., 80., 60.), fill())];
    let red = vec![(rect(10., 10., 80., 60.), hsla(0., 1., 0.5, 1.)), (rect(10., 10., 80., 2.), fill())];
    let each = vec![one.clone(), Vec::new(), red, one, Vec::new()];
    assert!(each_as_from_scratch(parent_with_children(Vec::new()), 1, each));
}
//! Hit testing под CSS transform: указатель переводится обратной матрицей в
//! непреобразованные hitboxes единственного prepaint поддерева.

use crate::hooks::IntoHooked;
use crate::paint::effects::transformed_element::Transformed;
use gpui::{
    App, Bounds, Hitbox, HitboxBehavior, IntoElement, MouseDownEvent, ParentElement, Pixels, Point,
    Styled, TestAppContext, TransformationMatrix, Window, canvas, div, point, px, size,
};
use std::{cell::RefCell, rc::Rc};

fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(w), px(h)))
}

fn matrix(rotation_scale: [[f32; 2]; 2], translation: [f32; 2]) -> TransformationMatrix {
    TransformationMatrix {
        rotation_scale,
        translation,
    }
}

/// Leaf whose prepaint inserts one hitbox and whose paint records hovered clicks.
fn leaf(
    insert: impl FnOnce(Bounds<Pixels>, &mut Window) -> Hitbox + 'static,
    clicked: Rc<RefCell<Vec<Point<Pixels>>>>,
) -> gpui::Canvas<Hitbox> {
    canvas(
        move |bounds, window, _: &mut App| insert(bounds, window),
        move |_, hitbox, window, _| {
            window.on_mouse_event::<MouseDownEvent>(move |event, phase, window, _| {
                if phase.bubble() && hitbox.is_hovered(window) {
                    clicked.borrow_mut().push(event.position);
                }
            });
        },
    )
}

fn clicks<E: IntoElement + 'static>(
    make: impl FnOnce(Rc<RefCell<Vec<Point<Pixels>>>>) -> E,
    points: &[(f32, f32)],
) -> Vec<(f32, f32)> {
    let clicked = Rc::new(RefCell::new(Vec::new()));
    let mut app = TestAppContext::single();
    let cx = app.add_empty_window();
    let element = clicked.clone();
    cx.draw(
        point(px(0.), px(0.)),
        size(px(100.), px(100.)),
        move |_, _| make(element).into_any_element(),
    );
    for &(x, y) in points {
        cx.simulate_mouse_down(
            point(px(x), px(y)),
            gpui::MouseButton::Left,
            Default::default(),
        );
    }
    let hits = clicked
        .borrow()
        .iter()
        .map(|p| (f32::from(p.x), f32::from(p.y)))
        .collect();
    app.quit();
    hits
}

fn test_vcx() -> crate::document::state::ValueCx<'static> {
    let doc = Box::leak(Box::new(crate::document::state::DocState::new(
        crate::style::css::Media::default(),
    )));
    doc.values()
}

fn transformed(lin: [[f32; 2]; 2], clicked: Rc<RefCell<Vec<Point<Pixels>>>>) -> gpui::Div {
    let child = leaf(
        |bounds, window| window.insert_hitbox(bounds, HitboxBehavior::Normal),
        clicked,
    )
    .w(px(40.))
    .h(px(20.))
    .into_any_element();
    let mut group = Transformed::new(child, test_vcx());
    group.lin = lin;
    div().size(px(100.)).child(group.hooked())
}

#[test]
fn rotated_element_is_hit_at_its_rendered_shape_not_its_layout_box() {
    // rotate(90deg) about the centre (20, 10) of a 40x20 box: x' = 30 - y,
    // y' = x - 10, so the rendered shape is [10, 30] x [-10, 30].
    let hits = clicks(
        |clicked| transformed([[0., -1.], [1., 0.]], clicked),
        &[(20., 25.), (35., 5.), (5., 5.)],
    );
    assert_eq!(hits, [(20., 25.)]);
}

#[test]
fn scaled_element_is_hit_through_the_inverse_scale() {
    // scale(2) about (20, 10): the 40x20 box renders over [-20, 60] x [-10, 30].
    let hits = clicks(
        |clicked| transformed([[2., 0.], [0., 2.]], clicked),
        &[(55., 25.), (65., 5.), (10., 5.)],
    );
    assert_eq!(hits, [(55., 25.), (10., 5.)]);
}

#[test]
fn nested_transforms_compose_and_a_singular_matrix_is_not_hittable() {
    let hits = clicks(
        |clicked| {
            let nested = clicked.clone();
            div()
                .size(px(100.))
                .child(
                    leaf(
                        |_, window| {
                            window.with_hitbox_transform(
                                matrix([[1., 0.], [0., 1.]], [50., 0.]),
                                |window| {
                                    window.with_hitbox_transform(
                                        matrix([[2., 0.], [0., 2.]], [0., 0.]),
                                        |window| {
                                            window.insert_hitbox(
                                                rect(0., 0., 10., 10.),
                                                HitboxBehavior::Normal,
                                            )
                                        },
                                    )
                                },
                            )
                        },
                        nested,
                    )
                    .size(px(100.)),
                )
                .child(
                    leaf(
                        |_, window| {
                            window.with_hitbox_transform(
                                matrix([[0., 0.], [0., 0.]], [0., 0.]),
                                |window| {
                                    window.insert_hitbox(
                                        rect(0., 0., 100., 100.),
                                        HitboxBehavior::Normal,
                                    )
                                },
                            )
                        },
                        clicked,
                    )
                    .absolute()
                    .size(px(100.)),
                )
        },
        &[(65., 15.), (5., 5.), (0., 0.), (75., 5.)],
    );
    assert_eq!(hits, [(65., 15.)]);
}

#[test]
fn parent_clip_applies_in_parent_space_and_inner_masks_are_local() {
    let hits = clicks(
        |clicked| {
            leaf(
                |_, window| {
                    window.with_content_mask(
                        Some(gpui::ContentMask {
                            bounds: rect(0., 0., 30., 30.),
                        }),
                        |window| {
                            window.with_hitbox_transform(
                                matrix([[1., 0.], [0., 1.]], [20., 0.]),
                                |window| {
                                    window.with_content_mask(
                                        Some(gpui::ContentMask {
                                            bounds: rect(0., 0., 20., 5.),
                                        }),
                                        |window| {
                                            window.insert_hitbox(
                                                rect(0., 0., 20., 20.),
                                                HitboxBehavior::Normal,
                                            )
                                        },
                                    )
                                },
                            )
                        },
                    )
                },
                clicked,
            )
            .size(px(100.))
        },
        &[(25., 2.), (35., 2.), (25., 10.)],
    );
    assert_eq!(hits, [(25., 2.)]);
}

//! Hitboxes одного source-prepaint следуют размещению краски в колонку и страницу.

use super::*;
use gpui::{CursorStyle, HitboxBehavior, MouseDownEvent, PaintSegmentSlice, TestAppContext};
use std::cell::{Cell, RefCell};

#[test]
fn one_source_prepaint_routes_click_and_cursor_to_column_and_page_not_source() {
    let prepaint_count = Rc::new(Cell::new(0));
    let paint_count = Rc::new(Cell::new(0));
    let clicked = Rc::new(RefCell::new(Vec::new()));
    let mut app = TestAppContext::single();
    let cx = app.add_empty_window();
    let placements: Rc<[PaintSegment]> = Rc::from([
        route(1, 100.),
        PaintSegment {
            destination: point(px(0.), px(100.)),
            ..route(2, 0.)
        },
    ]);
    cx.draw(point(px(0.), px(0.)), size(px(250.), px(250.)), |_, _| {
        let hit_routes = placements.clone();
        let paint_routes = placements.clone();
        let prepaint_count = prepaint_count.clone();
        let paint_count = paint_count.clone();
        let clicked = clicked.clone();
        canvas(
            move |_, window, cx| {
                window
                    .with_prepaint_segments(
                        rect(0., 0., 50., 50.),
                        hit_routes.clone(),
                        cx,
                        |window, _| {
                            prepaint_count.set(prepaint_count.get() + 1);
                            let slices = hit_routes
                                .iter()
                                .map(|placement| PaintSegmentSlice {
                                    placement: *placement,
                                    source_clip: rect(0., 0., 20., 20.),
                                })
                                .collect();
                            window
                                .with_paint_segment_slices(slices, |window| {
                                    window.insert_hitbox(
                                        rect(0., 0., 20., 20.),
                                        HitboxBehavior::Normal,
                                    )
                                })
                                .unwrap()
                        },
                    )
                    .unwrap()
            },
            move |_, hitbox, window, cx| {
                paint_count.set(paint_count.get() + 1);
                window
                    .record_paint_segments(
                        rect(0., 0., 50., 50.),
                        paint_routes.clone(),
                        cx,
                        |window, _| quad(window, rect(0., 0., 20., 20.)),
                    )
                    .unwrap();
                window.set_cursor_style(CursorStyle::PointingHand, &hitbox);
                window.on_mouse_event::<MouseDownEvent>(move |event, phase, window, _| {
                    if phase.bubble() && hitbox.is_hovered(window) {
                        clicked.borrow_mut().push(event.position);
                    }
                });
            },
        )
    });
    for position in [
        point(px(5.), px(5.)),
        point(px(105.), px(5.)),
        point(px(5.), px(105.)),
    ] {
        cx.simulate_mouse_move(position, None, Default::default());
        let expected = if position == point(px(5.), px(5.)) {
            CursorStyle::Arrow
        } else {
            CursorStyle::PointingHand
        };
        assert_eq!(
            cx.update(|window, _| window.hit_test_cursor_style()),
            expected
        );
        cx.simulate_mouse_down(position, gpui::MouseButton::Left, Default::default());
    }
    assert_eq!(
        *clicked.borrow(),
        [point(px(105.), px(5.)), point(px(5.), px(105.))]
    );
    assert_eq!(prepaint_count.get(), 1);
    assert_eq!(paint_count.get(), 1);
    app.quit();
}

#[test]
fn child_prepaint_observer_sees_the_overflow_scope_used_for_hit_projection() {
    use gpui::{AvailableSpace, IntoElement, ParentElement, Styled};
    let entered = Rc::new(Cell::new(0));
    let observed = entered.clone();
    let mut app = TestAppContext::single();
    let cx = app.add_empty_window();
    cx.draw(point(px(0.), px(0.)), size(px(250.), px(100.)), |_, _| {
        let entered = entered.clone();
        canvas(
            move |_, window, cx| {
                let child = canvas(
                    |_, window, _| {
                        window.insert_hitbox(rect(10., 0., 30., 20.), HitboxBehavior::Normal)
                    },
                    |_, hitbox, window, _| {
                        window.set_cursor_style(CursorStyle::PointingHand, &hitbox);
                    },
                );
                let mut root = gpui::div()
                    .w(px(20.))
                    .h(px(20.))
                    .overflow_hidden()
                    .child(child)
                    .into_any_element();
                root.layout_as_root(
                    size(
                        AvailableSpace::Definite(px(20.)),
                        AvailableSpace::Definite(px(20.)),
                    ),
                    window,
                    cx,
                );
                let observer = Rc::new(gpui::PrepaintTraversal {
                    enter_children: Rc::new(move |window| {
                        entered.set(entered.get() + 1);
                        assert!(window.paint_segment_clips().is_ok());
                    }),
                });
                window
                    .with_prepaint_segments(
                        rect(0., 0., 100., 100.),
                        Rc::from([route(1, 100.)]),
                        cx,
                        |window, cx| {
                            window.with_render_context(Some(observer), |window| {
                                root.prepaint_at(point(px(0.), px(0.)), window, cx);
                            });
                        },
                    )
                    .unwrap();
                root
            },
            |_, mut root, window, cx| root.paint(window, cx),
        )
    });
    for (x, expected) in [
        (15., CursorStyle::Arrow),
        (115., CursorStyle::PointingHand),
        (125., CursorStyle::Arrow),
    ] {
        cx.simulate_mouse_move(point(px(x), px(5.)), None, Default::default());
        assert_eq!(
            cx.update(|window, _| window.hit_test_cursor_style()),
            expected
        );
    }
    assert_eq!(observed.get(), 1);
    app.quit();
}

//! Идентичность host context в снимке и границы гарантированного unwind baseline.

use super::draw;
use gpui::{ContentMask, TransformationMatrix, canvas, point, px, size};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[test]
fn snapshot_restores_exact_geometry_and_shared_context_identity() {
    let captured = Rc::new(Cell::new(1u32));
    let source = captured.clone();
    let mask = ContentMask {
        bounds: gpui::Bounds::new(point(px(2.), px(3.)), size(px(20.), px(30.))),
    };
    let transform = TransformationMatrix {
        translation: [0.25, 0.75],
        ..TransformationMatrix::unit()
    };
    draw(|| {
        canvas(
            move |_, window, _| {
                let (snapshot, offset, saved_mask) =
                    window.with_render_context(Some(source), |window| {
                        window.with_exact_element_offset(point(px(0.25), px(0.75)), |window| {
                            window.with_transformation(transform, |window| {
                                window.with_content_mask(Some(mask), |window| {
                                    (
                                        window.paint_ctx(),
                                        window.element_offset(),
                                        window.content_mask(),
                                    )
                                })
                            })
                        })
                    });
                // element_offset — prepaint API; геометрию replay проверяем в этой фазе.
                // Снимок удерживает тот же Rc, а не копию значения на момент capture.
                captured.set(2);
                let caller = Rc::new(Cell::new(3u32));
                let original_transform = window.current_transformation();
                let original_offset = window.element_offset();
                let original_mask = window.content_mask();
                window.with_render_context(Some(caller.clone()), |window| {
                    window.with_render_context(
                        Some(Rc::new(String::from("caller-only"))),
                        |window| {
                            window
                                .with_paint_ctx(snapshot.clone(), |window| {
                                    assert!(Rc::ptr_eq(
                                        &window.render_context::<Cell<u32>>().unwrap(),
                                        &captured
                                    ));
                                    assert_eq!(
                                        window.render_context::<Cell<u32>>().unwrap().get(),
                                        2
                                    );
                                    assert!(window.render_context::<String>().is_none());
                                    assert_eq!(window.current_transformation(), transform);
                                    assert_eq!(window.element_offset(), offset);
                                    assert_eq!(window.content_mask(), saved_mask);
                                    window
                                        .with_paint_ctx(snapshot, |window| {
                                            assert!(Rc::ptr_eq(
                                                &window.render_context::<Cell<u32>>().unwrap(),
                                                &captured
                                            ));
                                        })
                                        .unwrap();
                                })
                                .unwrap();
                            assert!(Rc::ptr_eq(
                                &window.render_context::<Cell<u32>>().unwrap(),
                                &caller
                            ));
                            assert_eq!(
                                window.render_context::<String>().unwrap().as_str(),
                                "caller-only"
                            );
                        },
                    );
                });
                assert_eq!(window.current_transformation(), original_transform);
                assert_eq!(window.element_offset(), original_offset);
                assert_eq!(window.content_mask(), original_mask);
                assert!(window.render_context::<Cell<u32>>().is_none());
            },
            |_, (), _, _| {},
        )
    });
}

#[test]
fn typed_context_replay_restores_caller_on_unwind() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, _| {
                let empty = window.paint_ctx();
                let caller = Rc::new(17u32);
                window.with_render_context(Some(caller.clone()), |window| {
                    let failed = catch_unwind(AssertUnwindSafe(|| {
                        window
                            .with_paint_ctx(empty, |window| {
                                assert!(window.render_context::<u32>().is_none());
                                window.with_render_context(Some(Rc::new(23u32)), |_| {
                                    panic!("aborted paint job");
                                });
                            })
                            .unwrap();
                    }));
                    assert!(failed.is_err());
                    assert!(Rc::ptr_eq(
                        &window.render_context::<u32>().unwrap(),
                        &caller
                    ));
                });
                assert!(window.render_context::<u32>().is_none());
            },
        )
    });
}

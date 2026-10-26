//! Восстановление дробной геометрии, raster frame и хозяина при abort.

use super::draw;
use gpui::{Bounds, ContentMask, RasterFrame, TransformationMatrix, canvas, point, px, size};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[test]
fn full_snapshot_replays_fractional_raster_and_restores_nested_caller() {
    draw(|| {
        canvas(
            |_, window, _| {
                let frame = RasterFrame::new(
                    point(px(0.125), px(0.375)),
                    Some((
                        TransformationMatrix::unit(),
                        TransformationMatrix {
                            translation: [0.25, 0.75],
                            ..TransformationMatrix::unit()
                        },
                    )),
                );
                let host = Rc::new(7u32);
                let snapshot = window.with_render_context(Some(host.clone()), |window| {
                    window.with_raster_frame(frame.clone(), |window| {
                        window.with_scene_capture_scale(1.5, |window| {
                            window.with_rem_size(Some(px(23.5)), |window| {
                                (window.snapshot_paint_state(), window.scale_factor())
                            })
                        })
                    })
                });
                (snapshot, frame, host)
            },
            |_, ((snapshot, scale), frame, host), window, _| {
                let caller = window.raster_frame().clone();
                let caller_scale = window.scale_factor();
                let caller_rem = window.rem_size();
                let outer = window.snapshot_paint_state();
                window
                    .with_paint_state(snapshot, |window| {
                        assert_eq!(window.raster_frame().clone(), frame);
                        assert_eq!(window.scale_factor(), scale);
                        assert_eq!(window.rem_size(), px(23.5));
                        assert!(Rc::ptr_eq(&window.render_context::<u32>().unwrap(), &host));
                        window
                            .with_paint_state(outer, |window| {
                                assert_eq!(window.raster_frame().clone(), caller);
                                assert_eq!(window.scale_factor(), caller_scale);
                                assert_eq!(window.rem_size(), caller_rem);
                                assert!(window.render_context::<u32>().is_none());
                            })
                            .unwrap();
                        assert_eq!(window.raster_frame().clone(), frame);
                    })
                    .unwrap();
                assert_eq!(window.raster_frame().clone(), caller);
                assert_eq!(window.scale_factor(), caller_scale);
                assert_eq!(window.rem_size(), caller_rem);
                assert!(window.render_context::<u32>().is_none());
            },
        )
    });
}

#[test]
fn replay_unwind_restores_state_and_invalidates_pass() {
    draw(|| {
        canvas(
            |_, window, _| {
                window.with_exact_element_offset(point(px(0.25), px(0.75)), |window| {
                    window.with_transformation(
                        TransformationMatrix {
                            translation: [5.25, 7.75],
                            ..TransformationMatrix::unit()
                        },
                        |window| (window.snapshot_paint_state(), window.element_offset()),
                    )
                })
            },
            |_, (snapshot, _), window, _| {
                let caller_frame = window.raster_frame().clone();
                let caller_transform = window.current_transformation();
                let caller_mask = window.content_mask();
                let caller_scale = window.scale_factor();
                let caller_host = Rc::new(11u32);
                let pass = window.paint_pass_id();
                window.with_render_context(Some(caller_host.clone()), |window| {
                    let failed = catch_unwind(AssertUnwindSafe(|| {
                        window
                            .with_paint_state(snapshot, |window| {
                                assert_eq!(
                                    window.current_transformation().translation,
                                    [5.25, 7.75]
                                );
                                assert!(window.render_context::<u32>().is_none());
                                // A mutation without a lexical guard must also be rolled back.
                                window.replace_glyph_offset(point(px(8.25), px(9.75)));
                                window.with_scene_capture_scale(1.5, |window| {
                                    window.with_content_mask_replaced(
                                        ContentMask {
                                            bounds: Bounds::new(
                                                point(px(1.), px(2.)),
                                                size(px(3.), px(4.)),
                                            ),
                                        },
                                        |_| panic!("abort deferred paint"),
                                    );
                                });
                            })
                            .unwrap();
                    }));
                    assert!(failed.is_err());
                    assert!(!pass.is_live());
                    assert_eq!(window.raster_frame().clone(), caller_frame);
                    assert_eq!(window.current_transformation(), caller_transform);
                    assert_eq!(window.content_mask(), caller_mask);
                    assert_eq!(window.scale_factor(), caller_scale);
                    assert!(Rc::ptr_eq(
                        &window.render_context::<u32>().unwrap(),
                        &caller_host
                    ));
                });
            },
        )
    });
}

#[test]
fn offscreen_unwind_restores_scene_scope_and_capture_environment() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, _| {
                let frame = window.raster_frame().clone();
                let transform = window.current_transformation();
                let mask = window.content_mask();
                let scale = window.scale_factor();
                let outer_scope = window.paint_scope_id();
                let mut inner_scope = None;
                let failed = catch_unwind(AssertUnwindSafe(|| {
                    let _ = window.paint_scene_to_png(size(px(8.), px(9.)), 1.5, |window| {
                        inner_scope = Some(window.paint_scope_id());
                        window.with_transformation(
                            TransformationMatrix {
                                translation: [2.25, 3.75],
                                ..TransformationMatrix::unit()
                            },
                            |_| panic!("abort capture"),
                        );
                    });
                }));
                assert!(failed.is_err());
                assert!(!inner_scope.unwrap().is_live());
                assert!(outer_scope.is_live());
                assert_eq!(window.raster_frame().clone(), frame);
                assert_eq!(window.current_transformation(), transform);
                assert_eq!(window.content_mask(), mask);
                assert_eq!(window.scale_factor(), scale);
                assert!(!window.paint_pass_id().is_live());
            },
        )
    });
}

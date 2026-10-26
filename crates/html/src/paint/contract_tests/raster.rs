//! Точность raster consumers: подложки, явная геометрия и выбор полей replay.

use super::draw;
use crate::text::raster::{Background, ReplicatedBackground, background_offset};
use gpui::{
    Bounds, ChildPaintContext, RasterFrame, Style, TransformationMatrix, canvas, point, px, size,
};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[test]
fn raster_replay_retains_host_identity_while_general_replay_inherits_caller() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, _| {
                let captured = Rc::new(17u32);
                let caller = Rc::new(23u32);
                let mut frame = RasterFrame::new(point(px(0.125), px(0.375)), None);
                frame.exact_bounds = Some((
                    Bounds::new(point(px(0.0), px(0.0)), size(px(10.0), px(10.0))),
                    Bounds::new(point(px(0.125), px(0.375)), size(px(10.25), px(10.5))),
                ));
                frame.set_context(Some(captured.clone()));
                let (general, mut raster, full) =
                    window.with_raster_frame(frame.clone(), |window| {
                        (
                            window.paint_ctx(),
                            window.raster_paint_ctx(),
                            window.snapshot_paint_state(),
                        )
                    });
                let mut outer = RasterFrame::new(
                    point(px(4.25), px(8.75)),
                    Some((
                        TransformationMatrix::unit(),
                        TransformationMatrix {
                            translation: [0.25, 0.75],
                            ..TransformationMatrix::unit()
                        },
                    )),
                );
                outer.set_context(Some(caller.clone()));
                window.with_raster_frame(outer.clone(), |window| {
                    window
                        .with_paint_ctx(general, |window| {
                            assert_eq!(window.raster_frame().clone(), outer);
                            assert!(Rc::ptr_eq(
                                &window.raster_context::<u32>().unwrap(),
                                &caller
                            ));
                        })
                        .unwrap();
                    raster.set_fill_transform(outer.fill_transform);
                    let mut expected = frame.clone();
                    expected.fill_transform = outer.fill_transform;
                    window
                        .with_raster_paint_ctx(raster, |window| {
                            assert_eq!(window.raster_frame().clone(), expected);
                            assert!(Rc::ptr_eq(
                                &window.raster_context::<u32>().unwrap(),
                                &captured
                            ));
                        })
                        .unwrap();
                    assert_eq!(window.raster_frame().clone(), outer);
                    let failed = catch_unwind(AssertUnwindSafe(|| {
                        window
                            .with_paint_state(full, |window| {
                                assert_eq!(window.raster_frame().clone(), frame);
                                panic!("aborted host raster replay");
                            })
                            .unwrap();
                    }));
                    assert!(failed.is_err());
                    assert_eq!(window.raster_frame().clone(), outer);
                    assert!(Rc::ptr_eq(
                        &window.raster_context::<u32>().unwrap(),
                        &caller
                    ));
                });
            },
        )
    });
}

#[test]
fn replicated_table_fill_and_transparent_child_retain_containing_text_frame() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, _| {
                let style = Style {
                    snap_to_device_pixels: true,
                    background: Some(gpui::rgb(0xffffff).into()),
                    ..Style::default()
                };
                let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(50.0), px(50.0)));
                let exact = Bounds::new(point(px(0.125), px(0.375)), size(px(50.0), px(50.0)));
                let parent = ChildPaintContext {
                    style: &style,
                    bounds,
                    exact_bounds: Some((bounds, exact)),
                    metadata: None,
                };
                let initial = window.raster_frame().clone();
                crate::paint::raster::with_background(&parent, window, |window| {
                    let background = window.raster_context::<Background>().unwrap();
                    let baseline = point(px(20.0), px(20.0));
                    assert_eq!(
                        background_offset(window, baseline),
                        Some(point(px(0.125), px(0.375)))
                    );
                    assert_eq!(background_offset(window, point(px(80.0), px(80.0))), None);
                    let marker = ReplicatedBackground;
                    let replicated = ChildPaintContext {
                        metadata: Some(&marker),
                        ..parent
                    };
                    crate::paint::raster::with_background(&replicated, window, |window| {
                        assert!(Rc::ptr_eq(
                            &window.raster_context::<Background>().unwrap(),
                            &background
                        ));
                        assert_eq!(
                            background_offset(window, baseline),
                            Some(point(px(0.125), px(0.375)))
                        );
                    });
                    let transparent = Style {
                        background: None,
                        ..style.clone()
                    };
                    let child = ChildPaintContext {
                        style: &transparent,
                        metadata: None,
                        ..replicated
                    };
                    crate::paint::raster::with_background(&child, window, |window| {
                        assert!(Rc::ptr_eq(
                            &window.raster_context::<Background>().unwrap(),
                            &background
                        ));
                    });
                });
                assert_eq!(window.raster_frame().clone(), initial);
            },
        )
    });
}

#[test]
fn fill_consumer_uses_matching_unsnapped_transform_at_fractional_edges() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, _| {
                let style = Style {
                    snap_to_device_pixels: true,
                    ..Style::default()
                };
                let bounds = Bounds::new(point(px(0.0), px(1.0)), size(px(11.0), px(20.0)));
                let exact = Bounds::new(point(px(0.25), px(0.75)), size(px(10.5), px(20.5)));
                let actual = TransformationMatrix {
                    translation: [4.0, 8.0],
                    ..TransformationMatrix::unit()
                };
                let desired = TransformationMatrix {
                    translation: [4.25, 8.75],
                    ..TransformationMatrix::unit()
                };
                for (scale, right, bottom) in [(1.0, 15.0, 30.0), (1.25, 18.0, 35.0)] {
                    window.with_scene_capture_scale(scale, |window| {
                        let mut frame = window.raster_frame().clone();
                        frame.exact_bounds = Some((bounds, exact));
                        window.with_transformation(actual, |window| {
                            window.with_raster_frame(frame.clone(), |window| {
                                assert_eq!(
                                    crate::paint::raster::fill_bounds(&style, bounds, window),
                                    bounds
                                );
                            });
                            frame.fill_transform = Some((actual, desired));
                            window.with_raster_frame(frame.clone(), |window| {
                                // Округлённые рёбра устройства после обращения actual.
                                let expected = Bounds::from_corners(
                                    point(px(1.0 / scale), px(2.0 / scale)),
                                    point(px((right - 4.0) / scale), px((bottom - 8.0) / scale)),
                                );
                                assert_eq!(
                                    crate::paint::raster::fill_bounds(&style, bounds, window),
                                    expected
                                );
                            });
                            frame.fill_transform = Some((TransformationMatrix::unit(), desired));
                            window.with_raster_frame(frame, |window| {
                                assert_eq!(
                                    crate::paint::raster::fill_bounds(&style, bounds, window),
                                    bounds
                                );
                            });
                        });
                    });
                }
            },
        )
    });
}

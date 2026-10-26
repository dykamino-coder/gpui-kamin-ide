//! Выбор ближайшей содержащей подложки в совпадающей системе координат.

use super::draw;
use crate::text::raster::background_offset;
use gpui::{Bounds, ChildPaintContext, Style, TransformationMatrix, canvas, point, px, size};

#[test]
fn text_frame_search_skips_outside_bounds_and_different_transforms() {
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
                let exact = Bounds::new(point(px(0.125), px(0.375)), bounds.size);
                let outer = ChildPaintContext {
                    style: &style,
                    bounds,
                    exact_bounds: Some((bounds, exact)),
                    metadata: None,
                };
                crate::paint::raster::with_background(&outer, window, |window| {
                    let bounds = Bounds::new(point(px(10.0), px(10.0)), size(px(10.0), px(10.0)));
                    let exact = Bounds::new(point(px(10.25), px(10.75)), bounds.size);
                    let child = ChildPaintContext {
                        bounds,
                        exact_bounds: Some((bounds, exact)),
                        ..outer
                    };
                    let inside = point(px(15.0), px(15.0));
                    let outside = point(px(30.0), px(30.0));
                    crate::paint::raster::with_background(&child, window, |window| {
                        assert_eq!(
                            background_offset(window, inside),
                            Some(point(px(0.25), px(-0.25)))
                        );
                        assert_eq!(
                            background_offset(window, outside),
                            Some(point(px(0.125), px(0.375)))
                        );
                    });
                    let transform = TransformationMatrix {
                        translation: [8.0, 12.0],
                        ..TransformationMatrix::unit()
                    };
                    window.with_transformation(transform, |window| {
                        crate::paint::raster::with_background(&child, window, |window| {
                            assert_eq!(
                                background_offset(window, inside),
                                Some(point(px(0.25), px(0.75)))
                            );
                            assert_eq!(background_offset(window, outside), None);
                            let inverse = TransformationMatrix {
                                translation: [-8.0, -12.0],
                                ..TransformationMatrix::unit()
                            };
                            window.with_transformation(inverse, |window| {
                                assert_eq!(
                                    background_offset(window, inside),
                                    Some(point(px(0.125), px(0.375)))
                                );
                            });
                        });
                    });
                });
                assert_eq!(background_offset(window, point(px(15.0), px(15.0))), None);
            },
        )
    });
}

#[test]
fn box_child_callback_selects_background_inside_existing_opacity_scope() {
    use crate::text::raster::{Background, ReplicatedBackground};
    use gpui::{ParentElement, Styled, div};
    use std::{cell::Cell, rc::Rc};

    for (opacity, replicated, eligible) in
        [(1.0, false, true), (0.5, false, false), (1.0, true, false)]
    {
        let painted = Rc::new(Cell::new(false));
        let observed = painted.clone();
        draw(|| {
            let child = canvas(
                |_, _, _| (),
                move |_, (), window, _| {
                    assert_eq!(window.element_opacity(), opacity);
                    assert_eq!(window.raster_context::<Background>().is_some(), eligible);
                    observed.set(true);
                },
            )
            .size_full();
            let mut parent = div()
                .size(px(50.0))
                .bg(gpui::rgb(0xffffff))
                .opacity(opacity)
                .child(child);
            parent.style().snap_to_device_pixels = Some(true);
            if replicated {
                parent = parent.with_paint_metadata(ReplicatedBackground);
            }
            parent
        });
        assert!(painted.get());
    }
}

#[test]
fn background_scope_unwind_restores_outer_raster_identity() {
    use crate::text::raster::Background;
    use std::{
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
    };

    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, _| {
                let host = Rc::new(31u32);
                let mut frame = window.raster_frame().clone();
                frame.set_context(Some(host.clone()));
                window.with_raster_frame(frame.clone(), |window| {
                    let style = Style {
                        snap_to_device_pixels: true,
                        background: Some(gpui::rgb(0xffffff).into()),
                        ..Style::default()
                    };
                    let context = ChildPaintContext {
                        style: &style,
                        bounds: Bounds::new(point(px(0.0), px(0.0)), size(px(20.0), px(20.0))),
                        exact_bounds: None,
                        metadata: None,
                    };
                    let failed = catch_unwind(AssertUnwindSafe(|| {
                        crate::paint::raster::with_background(&context, window, |window| {
                            assert!(window.raster_context::<Background>().is_some());
                            panic!("aborted background scope");
                        });
                    }));
                    assert!(failed.is_err());
                    assert_eq!(window.raster_frame(), &frame);
                    assert!(window.raster_context::<Background>().is_none());
                    assert!(Rc::ptr_eq(&window.raster_context::<u32>().unwrap(), &host));
                });
            },
        )
    });
}

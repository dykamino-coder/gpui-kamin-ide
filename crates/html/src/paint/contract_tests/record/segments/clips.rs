//! Родительская граница clip остаётся в системе своего сегмента при переносе OOF.

use super::*;
use gpui::ContentMask;

#[test]
fn independent_child_moves_inside_its_ancestor_clip_copy() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let (_, records) = window
                    .record_paint_segments(
                        rect(0., 0., 90., 90.),
                        Rc::from([route(2, 100.)]),
                        cx,
                        |window, _| {
                            window.with_content_mask(
                                Some(ContentMask {
                                    bounds: rect(10., 0., 30., 90.),
                                }),
                                |window| {
                                    window
                                        .with_paint_segments(Rc::from([route(2, 120.)]), |window| {
                                            quad(window, rect(15., 5., 20., 10.));
                                        })
                                        .unwrap();
                                },
                            );
                        },
                    )
                    .unwrap();
                let scene = records.get(PaintSegmentId(2)).unwrap().scene();
                assert_eq!(scene.quads.len(), 1);
                let quad = &scene.quads[0];
                assert_eq!(quad.transformation.translation[0], 120.);
                assert_eq!(quad.content_mask.bounds.origin.x.0, 110.);
                assert_eq!(quad.content_mask.bounds.right().0, 140.);
            },
        )
    });
}

#[test]
fn source_culling_cannot_discard_a_child_moved_into_the_parent_clip() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let (_, records) = window
                    .record_paint_segments(
                        rect(0., 0., 90., 90.),
                        Rc::from([route(2, 100.)]),
                        cx,
                        |window, _| {
                            window.with_content_mask(
                                Some(ContentMask {
                                    bounds: rect(10., 0., 30., 90.),
                                }),
                                |window| {
                                    window
                                        .with_paint_segments(Rc::from([route(2, 80.)]), |window| {
                                            // Исходный clip исключает лист, но независимый
                                            // перенос вводит его в целевой экземпляр clip.
                                            quad(window, rect(45., 5., 10., 10.));
                                        })
                                        .unwrap()
                                },
                            )
                        },
                    )
                    .unwrap();
                let scene = records.get(PaintSegmentId(2)).unwrap().scene();
                assert_eq!(scene.quads.len(), 1);
                assert_eq!(scene.quads[0].transformation.translation[0], 80.);
                assert_eq!(scene.quads[0].content_mask.bounds.origin.x.0, 110.);
                assert_eq!(scene.quads[0].content_mask.bounds.right().0, 140.);
            },
        )
    });
}

#[test]
fn ancestor_clip_applies_after_the_shared_filter_group() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let extent = rect(0., 0., 90., 90.);
                let (_, records) = window
                    .record_paint_segments(extent, Rc::from([route(2, 100.)]), cx, |window, _| {
                        window.with_content_mask(
                            Some(ContentMask {
                                bounds: rect(10., 0., 30., 90.),
                            }),
                            |window| {
                                window.paint_group(
                                    extent,
                                    Corners::default(),
                                    3.,
                                    0.5,
                                    0,
                                    &[],
                                    None,
                                    None,
                                    |window| {
                                        window
                                            .with_paint_segments(
                                                Rc::from([route(2, 120.)]),
                                                |window| {
                                                    quad(window, rect(15., 5., 20., 10.));
                                                },
                                            )
                                            .unwrap();
                                    },
                                )
                            },
                        )
                    })
                    .unwrap();
                let scene = records.get(PaintSegmentId(2)).unwrap().scene();
                assert_eq!(scene.groups.len(), 1);
                let output = &scene.surfaces[0];
                assert_eq!(output.content_mask.bounds.origin.x.0, 110.);
                assert_eq!(output.content_mask.bounds.right().0, 140.);
                assert_eq!(output.opacity, 0.5);
                assert_eq!(scene.groups[0].blur_radius, 3.);
                let input = &scene.groups[0].scene.quads[0];
                assert_eq!(input.transformation.translation[0], 120.);
                assert!(input.content_mask.bounds.right().0 >= 155.);
            },
        )
    });
}

#[test]
fn replaced_clip_restores_the_enclosing_segment_scope() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let (_, records) = window
                    .record_paint_segments(
                        rect(0., 0., 90., 90.),
                        Rc::from([route(2, 100.)]),
                        cx,
                        |window, _| {
                            window.with_content_mask(
                                Some(ContentMask {
                                    bounds: rect(10., 0., 30., 90.),
                                }),
                                |window| {
                                    window
                                        .with_paint_segments(Rc::from([route(2, 120.)]), |window| {
                                            window.with_content_mask_replaced(
                                                ContentMask {
                                                    bounds: rect(0., 0., 90., 90.),
                                                },
                                                |window| {
                                                    quad(window, rect(15., 5., 20., 10.));
                                                },
                                            );
                                        })
                                        .unwrap();
                                    quad(window, rect(15., 5., 20., 10.));
                                },
                            )
                        },
                    )
                    .unwrap();
                let quads = &records.get(PaintSegmentId(2)).unwrap().scene().quads;
                assert_eq!(quads.len(), 2);
                assert_eq!(quads[0].content_mask.bounds.origin.x.0, 120.);
                assert_eq!(quads[0].content_mask.bounds.right().0, 210.);
                assert_eq!(quads[1].content_mask.bounds.origin.x.0, 110.);
                assert_eq!(quads[1].content_mask.bounds.right().0, 140.);
            },
        )
    });
}

#[test]
fn replaced_clip_unwind_restores_caller_and_rejects_partial_record() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let mask = window.content_mask();
                let scope = window.paint_scope_id();
                let result = window.record_paint_segments(
                    rect(0., 0., 90., 90.),
                    Rc::from([route(2, 100.)]),
                    cx,
                    |window, _| {
                        let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            window
                                .with_paint_segments(Rc::from([route(2, 120.)]), |window| {
                                    window.with_content_mask_replaced(
                                        ContentMask {
                                            bounds: rect(0., 0., 90., 90.),
                                        },
                                        |_| panic!("restore clip and selection"),
                                    );
                                })
                                .unwrap();
                        }));
                        assert!(failed.is_err());
                    },
                );
                assert_eq!(
                    result.err(),
                    Some(PaintRecordError::Identity(gpui::PaintError::ExpiredPass))
                );
                assert_eq!(window.content_mask(), mask);
                assert!(window.paint_scope_id() == scope);
                assert!(!window.paint_pass_id().is_live());
            },
        )
    });
}

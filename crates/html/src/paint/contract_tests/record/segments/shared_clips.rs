//! Исключение overflow по containing block сохраняется внутри общей группы эффектов.

use super::*;
use gpui::{ContentMask, PaintPlan, PaintSegmentSlice};

#[test]
fn external_containing_block_bypasses_overflow_inside_one_shared_filter() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let extent = rect(0., 0., 100., 100.);
                let (_, records) = window
                    .record_paint_segments(extent, Rc::from([route(1, 100.)]), cx, |window, _| {
                        let outside = window.paint_segment_clips().unwrap();
                        window.with_content_mask(
                            Some(ContentMask {
                                bounds: rect(10., 0., 20., 100.),
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
                                        quad(window, rect(15., 5., 10., 10.));
                                        window.paint_group(
                                            extent,
                                            Corners::default(),
                                            0.,
                                            0.75,
                                            0,
                                            &[],
                                            None,
                                            None,
                                            |window| {
                                                window
                                                    .with_paint_segments(
                                                        Rc::from([route(1, 120.)]),
                                                        |window| {
                                                            window
                                                                .with_paint_segment_clips(
                                                                    &outside,
                                                                    |window| {
                                                                        quad(
                                                                            window,
                                                                            rect(
                                                                                45., 20., 10., 10.,
                                                                            ),
                                                                        );
                                                                    },
                                                                )
                                                                .unwrap();
                                                        },
                                                    )
                                                    .unwrap();
                                            },
                                        );
                                    },
                                );
                            },
                        );
                    })
                    .unwrap();
                let scene = records.get(PaintSegmentId(1)).unwrap().scene();
                assert_eq!(scene.groups.len(), 2);
                assert_eq!(scene.surfaces.len(), 1);
                assert_eq!(scene.surfaces[0].opacity, 0.5);
                assert_eq!(scene.groups[1].blur_radius, 3.);
                let normal = &scene.groups[1].scene.quads[0];
                assert_eq!(normal.content_mask.bounds.origin.x.0, 110.);
                assert_eq!(normal.content_mask.bounds.right().0, 130.);
                let oof = &scene.groups[0].scene.quads[0];
                assert_eq!(oof.transformation.translation[0], 120.);
                assert!(oof.content_mask.bounds.right().0 >= 175.);
                assert!(
                    scene.groups[1].scene.surfaces[0]
                        .content_mask
                        .bounds
                        .right()
                        .0
                        >= 175.
                );
                assert!(scene.surfaces[0].content_mask.bounds.right().0 >= 175.);
            },
        )
    });
}

#[test]
fn immutable_group_is_admitted_by_its_own_slice_at_its_output() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let extent = rect(0., 0., 100., 100.);
                let (_, leaf) = window
                    .record_paint(extent, cx, |window, _| {
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
                                quad(window, rect(0., 15., 20., 20.));
                            },
                        );
                    })
                    .unwrap();
                let plan = PaintPlan::record(leaf);
                let (_, records) = window
                    .record_paint_segments(extent, Rc::from([route(1, 100.)]), cx, |window, _| {
                        window
                            .with_paint_segment_slices(
                                Rc::from([PaintSegmentSlice {
                                    placement: route(1, 100.),
                                    source_clip: rect(0., 0., 100., 20.),
                                }]),
                                |window| {
                                    window
                                        .paint_plan(
                                            &plan,
                                            PaintPlacement {
                                                source: point(px(0.), px(0.)),
                                                destination: point(px(0.), px(0.)),
                                                clip: extent,
                                            },
                                        )
                                        .unwrap();
                                },
                            )
                            .unwrap();
                    })
                    .unwrap();
                let scene = records.get(PaintSegmentId(1)).unwrap().scene();
                assert_eq!(scene.surfaces[0].content_mask.bounds.bottom().0, 20.);
                assert!(
                    scene.groups[0].scene.quads[0]
                        .content_mask
                        .bounds
                        .bottom()
                        .0
                        >= 35.
                );
            },
        )
    });
}

#[test]
fn independently_moved_layer_retains_its_order_at_the_destination() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let extent = rect(0., 0., 100., 100.);
                let (_, records) = window
                    .record_paint_segments(extent, Rc::from([route(1, 0.)]), cx, |window, _| {
                        window
                            .with_paint_segments(Rc::from([route(1, 100.)]), |window| {
                                quad(window, rect(0., 0., 20., 20.));
                            })
                            .unwrap();
                        window.paint_layer(rect(0., 0., 20., 20.), |window| {
                            window
                                .with_paint_segments(Rc::from([route(1, 100.)]), |window| {
                                    quad(window, rect(0., 0., 20., 20.));
                                })
                                .unwrap();
                        });
                    })
                    .unwrap();
                let quads = &records.get(PaintSegmentId(1)).unwrap().scene().quads;
                assert_eq!(quads.len(), 2);
                assert!(quads[0].order < quads[1].order);
            },
        )
    });
}

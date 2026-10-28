//! Допуск узла не ограничивает OOF; клип назначения охватывает общий фильтр.

use super::*;
use gpui::{ContentMask, PaintPlan, PaintSegmentSlice};

#[test]
fn independent_oof_admission_survives_the_normal_ancestors_slice() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let extent = rect(0., 0., 100., 100.);
                let (_, records) = window
                    .record_paint_segments(extent, Rc::from([route(1, 100.)]), cx, |window, _| {
                        window
                            .with_paint_segment_slices(
                                Rc::from([PaintSegmentSlice {
                                    placement: route(1, 100.),
                                    source_clip: rect(0., 0., 100., 20.),
                                }]),
                                |window| {
                                    window.paint_group(
                                        extent,
                                        Corners::default(),
                                        2.,
                                        0.5,
                                        0,
                                        &[],
                                        None,
                                        None,
                                        |window| {
                                            quad(window, rect(0., 5., 20., 10.));
                                            window
                                                .with_paint_segment_slices(
                                                    Rc::from([PaintSegmentSlice {
                                                        placement: route(1, 120.),
                                                        source_clip: rect(0., 60., 100., 30.),
                                                    }]),
                                                    |window| quad(window, rect(0., 65., 20., 10.)),
                                                )
                                                .unwrap();
                                        },
                                    );
                                },
                            )
                            .unwrap();
                    })
                    .unwrap();
                let scene = records.get(PaintSegmentId(1)).unwrap().scene();
                assert_eq!(scene.groups.len(), 1);
                assert_eq!(scene.surfaces.len(), 1);
                assert_eq!(scene.surfaces[0].opacity, 0.5);
                let input = &scene.groups[0].scene.quads;
                assert_eq!(input.len(), 2);
                assert_eq!(input[0].content_mask.bounds.bottom().0, 20.);
                assert_eq!(input[1].content_mask.bounds.origin.y.0, 60.);
                assert_eq!(input[1].content_mask.bounds.bottom().0, 90.);
                assert_eq!(input[1].transformation.translation[0], 120.);
            },
        )
    });
}

#[test]
fn oof_uses_its_containing_blocks_clip_chain_and_restores_the_normal_chain() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let (_, records) = window
                    .record_paint_segments(
                        rect(0., 0., 100., 100.),
                        Rc::from([route(1, 100.)]),
                        cx,
                        |window, _| {
                            let outside_cb = window.paint_segment_clips().unwrap();
                            window.with_content_mask(
                                Some(ContentMask {
                                    bounds: rect(10., 0., 20., 100.),
                                }),
                                |window| {
                                    let inside_cb = window.paint_segment_clips().unwrap();
                                    window
                                        .with_paint_segments(Rc::from([route(1, 120.)]), |window| {
                                            window
                                                .with_paint_segment_clips(&outside_cb, |window| {
                                                    quad(window, rect(45., 5., 10., 10.))
                                                })
                                                .unwrap();
                                            window
                                                .with_paint_segment_clips(&inside_cb, |window| {
                                                    quad(window, rect(0., 20., 10., 10.))
                                                })
                                                .unwrap();
                                        })
                                        .unwrap();
                                    quad(window, rect(15., 40., 10., 10.));
                                },
                            );
                        },
                    )
                    .unwrap();
                let quads = &records.get(PaintSegmentId(1)).unwrap().scene().quads;
                assert_eq!(quads.len(), 3);
                assert!(quads[0].content_mask.bounds.right().0 >= 175.);
                assert_eq!(quads[1].content_mask.bounds.origin.x.0, 110.);
                assert_eq!(quads[1].content_mask.bounds.right().0, 130.);
                assert_eq!(quads[2].content_mask.bounds.origin.x.0, 110.);
            },
        )
    });
}

#[test]
fn destination_clip_is_applied_to_the_shared_filter_output() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let (_, records) = window
                    .record_paint_segments(
                        rect(0., 0., 100., 100.),
                        Rc::from([route(1, 100.)]),
                        cx,
                        |window, _| {
                            window.paint_group(
                                rect(0., 0., 100., 100.),
                                Corners::default(),
                                3.,
                                0.5,
                                0,
                                &[],
                                None,
                                None,
                                |window| quad(window, rect(25., 0., 20., 10.)),
                            );
                        },
                    )
                    .unwrap();
                let plan = PaintPlan::record(records.get(PaintSegmentId(1)).unwrap().clone());
                let (_, output) = window
                    .record_paint(rect(0., 0., 250., 100.), cx, |window, _| {
                        window
                            .paint_plan(
                                &plan,
                                PaintPlacement {
                                    source: point(px(0.), px(0.)),
                                    destination: point(px(0.), px(0.)),
                                    clip: rect(100., 0., 30., 100.),
                                },
                            )
                            .unwrap();
                    })
                    .unwrap();
                let scene = output.scene();
                let surface = scene.surfaces.last().unwrap();
                assert_eq!(surface.content_mask.bounds.right().0, 130.);
                let group = &scene.groups[surface.group as usize - 1];
                let quad = &group.scene.quads[0];
                assert!(quad.content_mask.bounds.right().0 >= 145.);
            },
        )
    });
}

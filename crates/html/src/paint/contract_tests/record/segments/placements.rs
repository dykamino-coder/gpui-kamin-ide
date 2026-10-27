//! Один узел допускает несколько размещений, сохраняя единый общий scope.

use super::*;
use gpui::PaintSegmentSlice;

fn repeated() -> Rc<[PaintSegment]> {
    Rc::from([
        route(1, 100.),
        PaintSegment {
            ordinal: 1,
            ..route(1, 150.)
        },
    ])
}

#[test]
fn repeated_node_uses_pair_identity_and_each_placements_own_slice() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let mut calls = 0;
                let (_, records) = window
                    .record_paint_segments(rect(0., 0., 100., 100.), repeated(), cx, |window, _| {
                        calls += 1;
                        window
                            .with_paint_segment_slices(
                                Rc::from([
                                    PaintSegmentSlice {
                                        placement: repeated()[0],
                                        source_clip: rect(0., 0., 20., 10.),
                                    },
                                    PaintSegmentSlice {
                                        placement: repeated()[1],
                                        source_clip: rect(0., 10., 20., 10.),
                                    },
                                ]),
                                |window| quad(window, rect(0., 0., 20., 20.)),
                            )
                            .unwrap();
                    })
                    .unwrap();
                assert_eq!(calls, 1);
                let records = records.into_records();
                assert_eq!(records.len(), 1);
                let quads = &records[0].1.scene().quads;
                assert_eq!(quads.len(), 2);
                assert_eq!(quads[0].transformation.translation[0], 100.);
                assert_eq!(quads[0].content_mask.bounds.bottom().0, 10.);
                assert_eq!(quads[1].transformation.translation[0], 150.);
                assert_eq!(quads[1].content_mask.bounds.top().0, 10.);
                assert_eq!(quads[1].content_mask.bounds.bottom().0, 20.);
            },
        )
    });
}

#[test]
fn repeated_leaves_share_one_ancestor_effect_but_repeated_scopes_have_own_effects() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let extent = rect(0., 0., 100., 100.);
                let (_, shared) = window
                    .record_paint_segments(extent, repeated(), cx, |window, _| {
                        window
                            .with_paint_segments(Rc::from([]), |window| {
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
                                        window
                                            .with_paint_segments(repeated(), |window| {
                                                quad(window, rect(0., 0., 20., 20.))
                                            })
                                            .unwrap();
                                    },
                                );
                            })
                            .unwrap();
                    })
                    .unwrap();
                let shared = shared.get(PaintSegmentId(1)).unwrap().scene();
                assert_eq!(shared.groups.len(), 1);
                assert_eq!(shared.surfaces.len(), 1);
                assert_eq!(shared.groups[0].scene.quads.len(), 2);
                let (_, repeated) = window
                    .record_paint_segments(extent, Rc::from([route(1, 100.)]), cx, |window, _| {
                        window
                            .with_paint_segments(repeated(), |window| {
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
                                        quad(window, rect(0., 0., 20., 20.));
                                    },
                                );
                            })
                            .unwrap();
                    })
                    .unwrap();
                let repeated = repeated.get(PaintSegmentId(1)).unwrap().scene();
                assert_eq!(repeated.groups.len(), 2);
                assert_eq!(repeated.surfaces.len(), 2);
                assert_eq!(repeated.groups[0].scene.quads.len(), 1);
                assert_eq!(repeated.groups[1].scene.quads.len(), 1);
                assert_eq!(
                    repeated.groups[0].scene.quads[0].transformation.translation[0],
                    100.
                );
                assert_eq!(
                    repeated.groups[1].scene.quads[0].transformation.translation[0],
                    150.
                );
            },
        )
    });
}

#[test]
fn duplicate_pair_is_rejected_before_node_callback() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                window
                    .record_paint_segments(
                        rect(0., 0., 100., 100.),
                        Rc::from([route(1, 100.)]),
                        cx,
                        |window, _| {
                            let mut calls = 0;
                            assert_eq!(
                                window
                                    .with_paint_segments(
                                        Rc::from([route(1, 100.), route(1, 150.)]),
                                        |_| calls += 1
                                    )
                                    .err(),
                                Some(PaintRecordError::InvalidGeometry)
                            );
                            assert_eq!(calls, 0);
                        },
                    )
                    .unwrap();
            },
        )
    });
}

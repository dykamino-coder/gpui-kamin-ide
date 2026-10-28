//! Невалидная граница не вызывает источник и не меняет сцену назначения.

use super::*;

#[test]
fn rejects_duplicate_segments_and_nonfinite_placement_before_source_paint() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let mut calls = 0;
                for routes in [
                    Rc::from([route(1, 0.), route(1, 10.)]),
                    Rc::from([route(1, 0.), route(2, f32::INFINITY)]),
                ] {
                    assert_eq!(
                        window
                            .record_paint_segments(rect(0., 0., 90., 90.), routes, cx, |_, _| {
                                calls += 1
                            })
                            .err(),
                        Some(PaintRecordError::InvalidGeometry),
                    );
                }
                assert_eq!(calls, 0);
                assert_eq!(
                    window
                        .with_paint_segments(Rc::from([route(1, 0.)]), |_| ())
                        .err(),
                    Some(PaintRecordError::InvalidSegment)
                );
            },
        )
    });
}

#[test]
fn empty_selection_produces_an_empty_segment_without_consuming_another_source() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let (_, records) = window
                    .record_paint_segments(
                        rect(0., 0., 90., 90.),
                        Rc::from([route(1, 0.), route(2, 100.)]),
                        cx,
                        |window, _| {
                            window
                                .with_paint_segments(Rc::from([]), |window| {
                                    quad(window, rect(0., 0., 10., 10.))
                                })
                                .unwrap();
                            window
                                .with_paint_segments(Rc::from([route(2, 100.)]), |window| {
                                    quad(window, rect(0., 0., 10., 10.))
                                })
                                .unwrap();
                        },
                    )
                    .unwrap();
                assert!(
                    records
                        .get(PaintSegmentId(1))
                        .unwrap()
                        .scene()
                        .quads
                        .is_empty()
                );
                assert_eq!(
                    records.get(PaintSegmentId(2)).unwrap().scene().quads.len(),
                    1
                );
            },
        )
    });
}

#[test]
fn clip_chain_handle_cannot_cross_segment_capture_boundaries() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let extent = rect(0., 0., 90., 90.);
                let (clips, _) = window
                    .record_paint_segments(extent, Rc::from([route(1, 0.)]), cx, |window, _| {
                        window.paint_segment_clips().unwrap()
                    })
                    .unwrap();
                window
                    .record_paint_segments(extent, Rc::from([route(1, 0.)]), cx, |window, _| {
                        assert_eq!(
                            window.with_paint_segment_clips(&clips, |_| ()).err(),
                            Some(PaintRecordError::InvalidSegment)
                        );
                        quad(window, rect(0., 0., 10., 10.));
                    })
                    .unwrap();
            },
        )
    });
}

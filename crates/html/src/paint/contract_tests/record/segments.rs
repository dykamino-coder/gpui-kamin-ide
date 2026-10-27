//! Сегменты разделяются до композиции, сохраняя общие эффекты и порядок внутри.

use super::*;
use gpui::{Corners, PaintRecordError, PaintSegment, PaintSegmentId, canvas};
use std::rc::Rc;

mod admission;
mod backdrops;
mod clips;
mod deferred;
mod effects;
mod filter_inputs;
mod reuse;
mod scaled;
mod scopes;
mod shared_clips;
mod validation;

fn route(id: u64, x: f32) -> PaintSegment {
    PaintSegment {
        id: PaintSegmentId(id),
        source: point(px(0.), px(0.)),
        destination: point(px(x), px(0.)),
    }
}

#[test]
fn nested_scopes_are_shared_inside_each_independently_moved_segment() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let extent = rect(0., 0., 90., 90.);
                let mut calls = 0;
                let (_, records) = window
                    .record_paint_segments(
                        extent,
                        Rc::from([route(1, 0.), route(2, 100.)]),
                        cx,
                        |window, _| {
                            calls += 1;
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
                                            quad(window, rect(4., 5., 70., 13.));
                                            window
                                                .with_paint_segments(
                                                    Rc::from([route(2, 120.)]),
                                                    |window| {
                                                        quad(window, rect(6., 7., 70., 13.));
                                                    },
                                                )
                                                .unwrap();
                                            quad(window, rect(8., 9., 70., 13.));
                                        },
                                    );
                                },
                            );
                        },
                    )
                    .unwrap();
                assert_eq!(calls, 1);
                let first = records.get(PaintSegmentId(1)).unwrap().scene();
                let second = records.get(PaintSegmentId(2)).unwrap().scene();
                for scene in [first, second] {
                    assert_eq!(scene.groups.len(), 2);
                    assert_eq!(scene.surfaces.len(), 1);
                    assert_eq!(scene.surfaces[0].opacity, 0.5);
                    assert_eq!(scene.groups[1].blur_radius, 3.);
                    assert_eq!(scene.groups[1].scene.surfaces[0].opacity, 0.75);
                }
                assert_eq!(first.groups[0].scene.quads.len(), 2);
                let quads = &second.groups[0].scene.quads;
                assert_eq!(quads.len(), 3);
                assert_eq!(quads[0].transformation.translation[0], 100.);
                assert_eq!(quads[1].transformation.translation[0], 120.);
                assert_eq!(quads[2].transformation.translation[0], 100.);
                assert!(quads[0].order < quads[1].order);
                assert!(quads[1].order < quads[2].order);
                assert_eq!(second.groups[0].bounds.origin.x.0, 100.);
                assert_eq!(second.groups[1].bounds.origin.x.0, 100.);
                assert!(second.groups[0].bounds.right().0 >= 196.);
                assert!(second.surfaces[0].content_mask.bounds.right().0 >= 196.);
            },
        )
    });
}

#[test]
fn segment_selection_restores_after_unwind_and_rejects_unknown_identity() {
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
                            let failed =
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    window
                                        .with_paint_segments(Rc::from([route(2, 120.)]), |_| {
                                            panic!("source selection aborted")
                                        })
                                        .unwrap();
                                }));
                            assert!(failed.is_err());
                            let mut entered = false;
                            assert_eq!(
                                window
                                    .with_paint_segments(Rc::from([route(3, 0.)]), |_| {
                                        entered = true;
                                    })
                                    .err(),
                                Some(PaintRecordError::InvalidSegment)
                            );
                            assert!(!entered);
                            quad(window, rect(4., 5., 12., 13.));
                        },
                    )
                    .unwrap();
                for (id, x) in [(1, 0.), (2, 100.)] {
                    let quads = &records.get(PaintSegmentId(id)).unwrap().scene().quads;
                    assert_eq!(quads.len(), 1);
                    assert_eq!(quads[0].transformation.translation[0], x);
                }
            },
        )
    });
}

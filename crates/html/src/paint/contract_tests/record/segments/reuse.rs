//! Вложенный immutable record переносится целиком в выбранные сегменты.

use super::*;
use gpui::PaintPlan;

#[test]
fn existing_record_groups_are_reused_as_one_selected_subtree() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let extent = rect(0., 0., 90., 90.);
                let (_, record) = window
                    .record_paint(extent, cx, |window, _| {
                        window.paint_group(
                            extent,
                            Corners::default(),
                            0.,
                            0.5,
                            0,
                            &[],
                            None,
                            None,
                            |window| quad(window, rect(4., 5., 12., 13.)),
                        );
                    })
                    .unwrap();
                let (_, records) = window
                    .record_paint_segments(
                        extent,
                        Rc::from([route(1, 0.), route(2, 100.)]),
                        cx,
                        |window, _| {
                            window
                                .with_paint_segments(Rc::from([route(2, 120.)]), |window| {
                                    window
                                        .paint_plan(
                                            &PaintPlan::record(record),
                                            placement(0., 0., extent),
                                        )
                                        .unwrap();
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
                        .groups
                        .is_empty()
                );
                let second = records.get(PaintSegmentId(2)).unwrap().scene();
                assert_eq!(second.groups.len(), 1);
                assert_eq!(second.surfaces[0].opacity, 0.5);
                assert_eq!(second.groups[0].scene.quads.len(), 1);
                assert_eq!(
                    second.groups[0].scene.quads[0].transformation.translation[0],
                    120.
                );
            },
        )
    });
}

#[test]
fn immutable_filter_input_does_not_inherit_the_callers_output_clip() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let extent = rect(0., 0., 90., 90.);
                let (_, records) = window
                    .record_paint_segments(extent, Rc::from([route(2, 100.)]), cx, |window, cx| {
                        window.with_content_mask(
                            Some(gpui::ContentMask {
                                bounds: rect(10., 0., 30., 90.),
                            }),
                            |window| {
                                window
                                    .with_paint_segments(Rc::from([route(2, 120.)]), |window| {
                                        let (_, record) = window
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
                                                    |window| quad(window, rect(45., 5., 10., 10.)),
                                                );
                                            })
                                            .unwrap();
                                        window
                                            .paint_plan(
                                                &PaintPlan::record(record),
                                                placement(0., 0., extent),
                                            )
                                            .unwrap();
                                    })
                                    .unwrap();
                            },
                        );
                    })
                    .unwrap();
                let scene = records.get(PaintSegmentId(2)).unwrap().scene();
                assert_eq!(scene.groups.len(), 1);
                assert_eq!(scene.surfaces[0].content_mask.bounds.origin.x.0, 110.);
                assert_eq!(scene.surfaces[0].content_mask.bounds.right().0, 140.);
                assert!(scene.groups[0].scene.quads[0].content_mask.bounds.right().0 >= 175.);
            },
        )
    });
}

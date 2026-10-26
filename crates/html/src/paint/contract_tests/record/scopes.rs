//! Составные эффекты разделяют запись, но получают новые group ids и clips.

use super::*;
use gpui::{Corners, PaintGroupDescriptor, PaintPlan, PaintScopeDescriptor, canvas};

#[test]
fn shared_opacity_wraps_all_leaves_once_and_clip_follows_blur() {
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
                            0.75,
                            0,
                            &[],
                            None,
                            None,
                            |window| {
                                quad(window, rect(4., 5., 12., 13.));
                            },
                        );
                    })
                    .unwrap();
                let plan = PaintPlan::scope(
                    PaintScopeDescriptor::Group(PaintGroupDescriptor {
                        bounds: extent,
                        opacity: 0.5,
                        blur_radius: 3.,
                        ..Default::default()
                    }),
                    PaintPlan::sequence([
                        PaintPlan::record(record.clone()),
                        PaintPlan::record(record),
                    ]),
                );
                let (_, output) = window
                    .record_paint(extent, cx, |window, _| {
                        for x in [0., 30.] {
                            window
                                .paint_plan(&plan, placement(x, 0., rect(x + 8., 0., 4., 20.)))
                                .unwrap();
                        }
                    })
                    .unwrap();
                let scene = output.scene();
                assert_eq!(scene.groups.len(), 6);
                assert_eq!(scene.surfaces.len(), 2);
                for (i, x) in [0., 30.].into_iter().enumerate() {
                    let root = &scene.surfaces[i];
                    assert_eq!(root.group, (i * 3 + 3) as u32);
                    assert_eq!(root.opacity, 0.5);
                    assert_eq!(root.content_mask.bounds.origin.x.0, x + 8.);
                    assert_eq!(root.content_mask.bounds.size.width.0, 4.);
                    let group = &scene.groups[i * 3 + 2];
                    assert_eq!(group.blur_radius, 3.);
                    assert_eq!(group.scene.surfaces.len(), 2);
                    assert_eq!(group.scene.surfaces[0].group, (i * 3 + 1) as u32);
                    assert_eq!(group.scene.surfaces[1].group, (i * 3 + 2) as u32);
                    // Blur input is not cut down to the four-pixel fragment output.
                    assert_eq!(
                        scene.groups[i * 3].scene.quads[0]
                            .content_mask
                            .bounds
                            .size
                            .width
                            .0,
                        90.
                    );
                }
            },
        )
    });
}

#[test]
fn source_clip_and_closed_capture_snapshot_have_distinct_lifetimes() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let extent = rect(0., 0., 90., 90.);
                let (snapshot, record) = window
                    .record_paint(extent, cx, |window, _| {
                        quad(window, rect(0., 0., 20., 20.));
                        window.snapshot_paint_state()
                    })
                    .unwrap();
                assert_eq!(
                    window.with_paint_state(snapshot, |_| ()).err(),
                    Some(gpui::PaintError::ExpiredScope)
                );
                let plan = PaintPlan::scope(
                    PaintScopeDescriptor::Clip(rect(2., 3., 5., 6.)),
                    PaintPlan::record(record),
                );
                let (_, output) = window
                    .record_paint(extent, cx, |window, _| {
                        window
                            .paint_plan(&plan, placement(10., 20., extent))
                            .unwrap();
                    })
                    .unwrap();
                let clip = output.scene().quads[0].content_mask.bounds;
                assert_eq!(clip.origin.x.0, 12.);
                assert_eq!(clip.origin.y.0, 23.);
                assert_eq!(clip.size.width.0, 5.);
                assert_eq!(clip.size.height.0, 6.);
            },
        )
    });
}

#[test]
fn depth_and_bottom_order_are_recomputed_by_the_same_scene_backend() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let extent = rect(0., 0., 90., 90.);
                let (_, record) = window
                    .record_paint(extent, cx, |window, _| {
                        window.paint_depth_context(|window| {
                            for (z, width) in [(10., 10.), (-10., 20.)] {
                                window.paint_depth_plane(z, |window| {
                                    quad(window, rect(0., 0., width, 10.))
                                });
                            }
                        });
                        window.paint_bottom_layer(|window| {
                            quad(window, rect(0., 0., 30., 10.));
                            quad(window, rect(0., 0., 40., 10.));
                        });
                    })
                    .unwrap();
                let (_, output) = window
                    .record_paint(extent, cx, |window, _| {
                        window
                            .paint_plan(
                                &PaintPlan::record(record.clone()),
                                placement(3., 4., extent),
                            )
                            .unwrap();
                    })
                    .unwrap();
                let source: Vec<_> = record
                    .scene()
                    .quads
                    .iter()
                    .map(|q| (q.bounds.size, q.order))
                    .collect();
                let target: Vec<_> = output
                    .scene()
                    .quads
                    .iter()
                    .map(|q| (q.bounds.size, q.order))
                    .collect();
                assert_eq!(source, target);
                assert!(
                    source.iter().position(|v| v.0.width.0 == 20.).unwrap()
                        < source.iter().position(|v| v.0.width.0 == 10.).unwrap()
                );
            },
        )
    });
}

#[test]
fn deeply_nested_plan_uses_heap_for_execution_and_drop() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let extent = rect(0., 0., 90., 90.);
                let (_, record) = window
                    .record_paint(extent, cx, |window, _| quad(window, rect(1., 1., 10., 10.)))
                    .unwrap();
                let mut plan = PaintPlan::record(record);
                for _ in 0..1026 {
                    plan = PaintPlan::scope(
                        PaintScopeDescriptor::Group(PaintGroupDescriptor {
                            bounds: extent,
                            ..Default::default()
                        }),
                        plan,
                    );
                }
                window.paint_plan(&plan, placement(0., 0., extent)).unwrap();
                drop(plan);
            },
        )
    });
}

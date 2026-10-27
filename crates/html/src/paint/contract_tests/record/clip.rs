//! Внешний clip ограничивает composite, внутренний clip ограничивает вход группы.

use super::*;
use gpui::{PaintGroupDescriptor, PaintPlan, PaintScopeDescriptor, canvas};

#[test]
fn ancestor_clip_cuts_group_output_without_discarding_filter_input() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let extent = rect(0., 0., 90., 90.);
                let clip = rect(8., 0., 4., 20.);
                let (_, record) = window
                    .record_paint(extent, cx, |window, _| {
                        quad(window, rect(4., 5., 12., 13.));
                    })
                    .unwrap();
                let group = PaintScopeDescriptor::Group(PaintGroupDescriptor {
                    bounds: extent,
                    blur_radius: 3.,
                    ..Default::default()
                });
                let leaf = PaintPlan::record(record);
                let outer_clip = PaintPlan::scope(
                    PaintScopeDescriptor::Clip(clip),
                    PaintPlan::scope(group.clone(), leaf.clone()),
                );
                let inner_clip = PaintPlan::scope(
                    group,
                    PaintPlan::scope(PaintScopeDescriptor::Clip(clip), leaf),
                );
                let (_, output) = window
                    .record_paint(extent, cx, |window, _| {
                        for plan in [&outer_clip, &inner_clip] {
                            window.paint_plan(plan, placement(0., 0., extent)).unwrap();
                        }
                    })
                    .unwrap();
                let scene = output.scene();
                assert_eq!(scene.surfaces.len(), 2);
                assert_eq!(scene.groups.len(), 2);
                assert_eq!(scene.surfaces[0].content_mask.bounds, clip.scale(1.));
                assert_eq!(scene.surfaces[1].content_mask.bounds, extent.scale(1.));
                assert_eq!(scene.groups[0].scene.quads.len(), 1);
                assert_eq!(scene.groups[1].scene.quads.len(), 1);
                assert_eq!(
                    scene.groups[0].scene.quads[0].content_mask.bounds,
                    extent.scale(1.)
                );
                assert_eq!(
                    scene.groups[1].scene.quads[0].content_mask.bounds,
                    clip.scale(1.)
                );
            },
        )
    });
}

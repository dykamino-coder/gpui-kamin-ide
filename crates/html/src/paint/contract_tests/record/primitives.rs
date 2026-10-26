//! Перенос растровых ресурсов, путей и эффектов использует исходные команды.

use super::*;
use gpui::{
    App, BoxShadow, Corners, PaintGroupDescriptor, PaintPlan, PaintScopeDescriptor, Path,
    RenderImage, UnderlineStyle, canvas,
};
use std::sync::Arc;

fn image() -> Arc<RenderImage> {
    Arc::new(RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::from_pixel(2, 2, image::Rgba([10, 20, 30, 255])),
    )]))
}

fn source(window: &mut Window, _: &mut App) {
    let bounds = rect(5., 6., 20., 20.);
    window.paint_drop_shadows(
        bounds,
        Corners::default(),
        &[BoxShadow::new(px(2.), px(3.), rgb(0).into()).blur_radius(px(4.))],
    );
    window.paint_underline(
        point(px(5.), px(6.)),
        px(20.),
        &UnderlineStyle {
            thickness: px(2.),
            color: Some(rgb(0).into()),
            wavy: true,
        },
    );
    let mut path = Path::new(point(px(5.), px(6.)));
    path.line_to(point(px(25.), px(6.)));
    path.line_to(point(px(25.), px(26.)));
    path.line_to(point(px(5.), px(6.)));
    window.paint_path(path, rgb(0));
    window
        .paint_image(bounds, bounds, Corners::default(), image(), 0, false)
        .unwrap();
    window.paint_backdrop_filter(bounds, Corners::default(), 2., [0.; 20], false);
    window.paint_group(
        rect(0., 0., 90., 90.),
        Corners::default(),
        0.,
        1.,
        0,
        &[],
        None,
        None,
        |window| {
            window.paint_backdrop_filter(bounds, Corners::default(), 2., [0.; 20], true);
            quad(window, bounds);
        },
    );
}

#[test]
fn command_translation_keeps_tiles_uvs_effects_and_backdrop_order() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let extent = rect(0., 0., 90., 90.);
                let (_, record) = window.record_paint(extent, cx, source).unwrap();
                let (_, output) = window
                    .record_paint(extent, cx, |window, _| {
                        window
                            .paint_plan(
                                &PaintPlan::record(record.clone()),
                                placement(7., 9., extent),
                            )
                            .unwrap();
                    })
                    .unwrap();
                let a = record.scene();
                let b = output.scene();
                let delta = point(gpui::ScaledPixels(7.), gpui::ScaledPixels(9.));
                assert_eq!(a.shadows.len(), 1);
                assert_eq!(
                    b.shadows[0].bounds.origin,
                    a.shadows[0].bounds.origin + delta
                );
                assert_eq!(
                    b.shadows[0].element_bounds.origin,
                    a.shadows[0].element_bounds.origin + delta
                );
                assert_eq!(b.shadows[0].blur_radius, a.shadows[0].blur_radius);
                assert_eq!(
                    b.underlines[0].bounds.origin,
                    a.underlines[0].bounds.origin + delta
                );
                assert_eq!(a.paths.len(), 1);
                for (old, new) in a.paths[0].vertices.iter().zip(&b.paths[0].vertices) {
                    assert_eq!(new.xy_position, old.xy_position + delta);
                    assert_eq!(new.st_position, old.st_position);
                }
                assert_eq!(a.paths[0].color, b.paths[0].color);
                assert_eq!(a.polychrome_sprites.len(), 1);
                assert_eq!(a.polychrome_sprites[0].tile, b.polychrome_sprites[0].tile);
                assert_eq!(
                    a.polychrome_sprites[0].bounds,
                    b.polychrome_sprites[0].bounds
                );
                assert_eq!(b.polychrome_sprites[0].transformation.translation, [7., 9.]);
                assert_eq!(b.surfaces.len(), 2);
                assert_eq!(b.surfaces[0].group, 0);
                assert_eq!(b.surfaces[0].color_matrix, a.surfaces[0].color_matrix);
                assert!(b.surfaces[0].order < b.surfaces[1].order);
            },
        )
    });
}

#[test]
fn reopenable_group_retains_mask_projective_map_and_coverage() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let extent = rect(0., 0., 90., 90.);
                let (_, record) = window
                    .record_paint(extent, cx, |window, _| quad(window, rect(2., 3., 4., 5.)))
                    .unwrap();
                let mask = image();
                let polygon = vec![
                    point(px(0.), px(0.)),
                    point(px(10.), px(0.)),
                    point(px(0.), px(10.)),
                ];
                let descriptor = PaintGroupDescriptor {
                    bounds: extent,
                    polygon: polygon.clone(),
                    polygon_inverse: true,
                    mask: Some((mask.clone(), rect(2., 3., 20., 30.), 3)),
                    mask_clip: Some([2., 3., 20., 30.]),
                    projection: Some([1., 0., 2., 0., 1., 3., 0., 0., 1.]),
                    fragments: vec![polygon],
                    ..Default::default()
                };
                let plan = PaintPlan::scope(
                    PaintScopeDescriptor::Group(descriptor),
                    PaintPlan::record(record),
                );
                let (_, output) = window
                    .record_paint(extent, cx, |window, _| {
                        window.paint_plan(&plan, placement(7., 9., extent)).unwrap();
                    })
                    .unwrap();
                let group = &output.scene().groups[0];
                assert!(Arc::ptr_eq(group.mask.as_ref().unwrap(), &mask));
                assert_eq!(
                    group.mask_bounds.origin,
                    point(gpui::ScaledPixels(9.), gpui::ScaledPixels(12.))
                );
                assert_eq!(group.mask_clip, Some([9., 12., 20., 30.]));
                assert_eq!(group.mask_once, 3);
                assert!(group.polygon_inverse);
                assert_eq!(
                    group.polygon[0],
                    point(gpui::ScaledPixels(7.), gpui::ScaledPixels(9.))
                );
                assert_eq!(group.fragments[0], group.polygon);
                assert_eq!(group.projection, Some([1., 0., 2., 0., 1., 3., 0., 0., 1.]));
            },
        )
    });
}

//! Векторное представление страницы меняет команды до растеризации и восстанавливает scope.

use super::*;
use gpui::{
    ContentMask, Corners, PaintVectorTransform, Path, TransformationMatrix, UnderlineStyle, canvas,
};

#[test]
fn page_vector_scope_rotates_paths_masks_and_underlines_at_each_density() {
    for density in [1., 1.25, 2.] {
        super::super::draw_at_scale(Some(density), move || {
            canvas(
                |_, _, _| (),
                move |_, (), window, cx| {
                    let extent = rect(0., 0., 90., 90.);
                    let page = PaintVectorTransform::new(0.5, 1, point(px(50.), px(10.))).unwrap();
                    let (_, output) = window
                        .record_paint(extent, cx, |window, _| {
                            window.with_paint_vector_transform(page, |window| {
                                window.with_content_mask(
                                    Some(ContentMask {
                                        bounds: rect(2., 4., 20., 30.),
                                    }),
                                    |window| {
                                        quad(window, rect(4., 6., 8., 10.));
                                        let mut path = Path::new(point(px(4.), px(6.)));
                                        path.line_to(point(px(12.), px(6.)));
                                        path.line_to(point(px(12.), px(16.)));
                                        path.line_to(point(px(4.), px(6.)));
                                        window.paint_path(path, rgb(0x123456));
                                        window.paint_underline(
                                            point(px(4.), px(6.)),
                                            px(8.),
                                            &UnderlineStyle {
                                                thickness: px(2.),
                                                color: Some(rgb(0).into()),
                                                wavy: true,
                                            },
                                        );
                                    },
                                );
                            });
                        })
                        .unwrap();
                    let scene = output.scene();
                    assert_eq!(scene.quads.len(), 1);
                    let quad = &scene.quads[0];
                    assert_eq!(quad.transformation.rotation_scale, [[0., -0.5], [0.5, 0.]]);
                    assert_eq!(
                        quad.transformation.translation,
                        [50. * density, 10. * density]
                    );
                    assert_eq!(
                        quad.content_mask.bounds,
                        rect(33., 11., 15., 10.).scale(density)
                    );
                    assert_eq!(scene.paths.len(), 1);
                    let path = &scene.paths[0];
                    assert_eq!(path.bounds, rect(42., 12., 5., 4.).scale(density));
                    let (color_bounds, inverse) = path.color_mapping.unwrap();
                    assert_eq!(color_bounds, rect(4., 6., 8., 10.).scale(density));
                    let restored = inverse.apply(point(px(47. * density), px(12. * density)));
                    assert_eq!(restored, point(px(4. * density), px(6. * density)));
                    assert_eq!(scene.underlines.len(), 1);
                    assert_eq!(scene.underlines[0].pad, 1);
                    assert_eq!(scene.underlines[0].thickness.0, density);
                    assert_eq!(
                        window.current_transformation(),
                        TransformationMatrix::unit()
                    );
                },
            )
        });
    }
}

#[test]
fn vector_scope_transforms_flat_group_dependencies_and_restores_ambient_css() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let extent = rect(0., 0., 90., 90.);
                let page = PaintVectorTransform::new(0.5, 1, point(px(50.), px(10.))).unwrap();
                let css = TransformationMatrix {
                    translation: [3., 7.],
                    ..TransformationMatrix::unit()
                };
                let (_, output) = window
                    .record_paint(extent, cx, |window, _| {
                        window.with_transformation(css, |window| {
                            let outer_mask = window.content_mask();
                            window.with_paint_vector_transform(page, |window| {
                                window.paint_group(
                                    rect(2., 4., 20., 30.),
                                    Corners::default(),
                                    4.,
                                    0.5,
                                    0,
                                    &[
                                        point(px(2.), px(4.)),
                                        point(px(22.), px(4.)),
                                        point(px(2.), px(34.)),
                                    ],
                                    None,
                                    Some([2., 4., 20., 30.]),
                                    |window| {
                                        window.paint_group(
                                            rect(4., 6., 8., 10.),
                                            Corners::default(),
                                            0.,
                                            0.5,
                                            0,
                                            &[],
                                            None,
                                            None,
                                            |window| quad(window, rect(4., 6., 8., 10.)),
                                        );
                                    },
                                );
                            });
                            assert_eq!(window.current_transformation(), css);
                            assert_eq!(window.content_mask(), outer_mask);
                        });
                    })
                    .unwrap();
                let scene = output.scene();
                assert_eq!(scene.groups.len(), 2);
                assert_eq!(scene.groups[0].scene.quads.len(), 1);
                assert_eq!(
                    scene.groups[0].scene.quads[0].transformation.translation,
                    [50., 10.]
                );
                let group = &scene.groups[1];
                assert_eq!(group.bounds, rect(33., 11., 15., 10.).scale(1.));
                assert_eq!(group.blur_radius, 2.);
                assert_eq!(group.mask_clip, Some([33., 11., 15., 10.]));
                assert_eq!(
                    group.polygon[0],
                    point(gpui::ScaledPixels(48.), gpui::ScaledPixels(11.))
                );
                assert_eq!(group.scene.surfaces[0].group, 1);
                assert_eq!(scene.surfaces[0].group, 2);
            },
        )
    });
}

#[test]
fn page_transform_inverse_handles_small_and_large_finite_scale_without_squaring() {
    for scale in [1e-20, 1e20] {
        let page = PaintVectorTransform::new(scale, 1, point(px(0.), px(0.))).unwrap();
        let p = point(px(2.), px(4.));
        assert_eq!(page.inverse_point(page.point(p)), p);
    }
    assert!(PaintVectorTransform::new(0., 0, point(px(0.), px(0.))).is_none());
    assert!(PaintVectorTransform::new(f32::NAN, 0, point(px(0.), px(0.))).is_none());
}

//! Явный презентационный scope (лист страницы, transform копии) переносит команды записи
//! при воспроизведении; без него вызывающий transform не применяется.

use super::*;
use gpui::{
    ContentMask, PaintPlan, PaintRecordError, Path, TransformationMatrix, UnderlineStyle, canvas,
};

fn page() -> TransformationMatrix {
    // Uniform scale 0.5 and one clockwise quarter-turn, in logical pixels.
    TransformationMatrix {
        rotation_scale: [[0., -0.5], [0.5, 0.]],
        translation: [50., 10.],
    }
}

pub(super) fn source(window: &mut Window) {
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
}

#[test]
fn presentation_scope_moves_quads_paths_masks_and_underlines_at_each_density() {
    for density in [1., 1.25, 2.] {
        super::super::draw_at_scale(Some(density), move || {
            canvas(
                |_, _, _| (),
                move |_, (), window, cx| {
                    let extent = rect(0., 0., 90., 90.);
                    let (_, record) = window
                        .record_paint(extent, cx, |window, _| source(window))
                        .unwrap();
                    let plan = PaintPlan::record(record.clone());
                    let (_, output) = window
                        .record_paint(extent, cx, |window, _| {
                            window
                                .with_paint_presentation(page(), |window| {
                                    window.paint_plan(&plan, placement(0., 0., extent)).unwrap();
                                })
                                .unwrap();
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
                    let path = &scene.paths[0];
                    assert_eq!(path.bounds, rect(42., 12., 5., 4.).scale(density));
                    let (color_bounds, inverse) = path.color_mapping.unwrap();
                    assert_eq!(color_bounds, rect(4., 6., 8., 10.).scale(density));
                    let restored = inverse.apply(point(px(47. * density), px(12. * density)));
                    assert_eq!(restored, point(px(4. * density), px(6. * density)));
                    assert_eq!(scene.underlines[0].pad, 1);
                    assert_eq!(
                        scene.underlines[0].thickness.0,
                        record.scene().underlines[0].thickness.0 * 0.5
                    );
                },
            )
        });
    }
}

#[test]
fn presentation_scope_leaves_live_paint_capture_and_host_scopes_alone() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let extent = rect(0., 0., 90., 90.);
                let (_, outside) = window
                    .record_paint(extent, cx, |window, _| source(window))
                    .unwrap();
                let (_, inside) = window
                    .with_paint_presentation(page(), |window| {
                        window.record_paint(extent, cx, |window, _| source(window))
                    })
                    .unwrap()
                    .unwrap();
                let (a, b) = (outside.scene(), inside.scene());
                assert_eq!(a.quads[0].transformation, b.quads[0].transformation);
                assert_eq!(a.quads[0].content_mask, b.quads[0].content_mask);
                assert_eq!(a.paths[0].bounds, b.paths[0].bounds);
                let scoped = PaintPlan::scope(
                    gpui::PaintScopeDescriptor::Clip(extent),
                    PaintPlan::record(outside),
                );
                let rejected = window
                    .with_paint_presentation(page(), |window| {
                        window.paint_plan(&scoped, placement(0., 0., extent)).err()
                    })
                    .unwrap();
                assert_eq!(rejected, Some(PaintRecordError::InvalidGeometry));
            },
        )
    });
}

#[test]
fn presentation_scope_rejects_a_singular_matrix() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, _| {
                for matrix in [
                    TransformationMatrix {
                        rotation_scale: [[0., 0.], [0., 0.]],
                        translation: [0., 0.],
                    },
                    TransformationMatrix {
                        rotation_scale: [[1., 2.], [2., 4.]],
                        translation: [0., 0.],
                    },
                    TransformationMatrix {
                        rotation_scale: [[f32::NAN, 0.], [0., 1.]],
                        translation: [0., 0.],
                    },
                ] {
                    let entered = window.with_paint_presentation(matrix, |_| ());
                    assert_eq!(entered.err(), Some(PaintRecordError::InvalidGeometry));
                }
            },
        )
    });
}

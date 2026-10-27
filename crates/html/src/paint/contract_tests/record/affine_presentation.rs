//! Воспроизведение записи под аффинной презентацией (transform копии предков) даёт те же
//! видимые пиксели, что и прямая live-отрисовка поддерева под той же матрицей.

use super::vector_transform::source;
use super::*;
use gpui::{Bounds, PaintPlan, ScaledPixels, TransformationMatrix, canvas};

/// Device bounds of `b` under `m` (four corners).
fn placed(b: Bounds<ScaledPixels>, m: &TransformationMatrix) -> Bounds<ScaledPixels> {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for (x, y) in [
        (b.origin.x.0, b.origin.y.0),
        (b.right().0, b.origin.y.0),
        (b.origin.x.0, b.bottom().0),
        (b.right().0, b.bottom().0),
    ] {
        let p = m.apply(point(px(x), px(y)));
        let (x, y) = (f32::from(p.x), f32::from(p.y));
        (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
    }
    Bounds::new(
        point(ScaledPixels(x0), ScaledPixels(y0)),
        gpui::size(ScaledPixels(x1 - x0), ScaledPixels(y1 - y0)),
    )
}

/// Non-similarity CSS transforms of a copy, in logical pixels.
fn affine() -> [TransformationMatrix; 3] {
    let (cos, sin) = (30f32.to_radians().cos(), 30f32.to_radians().sin());
    [
        // scaleY(2) about a bottom origin.
        TransformationMatrix {
            rotation_scale: [[1., 0.], [0., 2.]],
            translation: [0., -40.],
        },
        // rotate(30deg).
        TransformationMatrix {
            rotation_scale: [[cos, -sin], [sin, cos]],
            translation: [20., 5.],
        },
        // skewX(20deg).
        TransformationMatrix {
            rotation_scale: [[1., 20f32.to_radians().tan()], [0., 1.]],
            translation: [3., 0.],
        },
    ]
}

#[test]
fn presentation_replay_matches_live_paint_under_any_affine_matrix() {
    for density in [1., 1.25, 2.] {
        super::super::draw_at_scale(Some(density), move || {
            canvas(
                |_, _, _| (),
                move |_, (), window, cx| {
                    let extent = rect(0., 0., 90., 90.);
                    let (_, record) = window
                        .record_paint(extent, cx, |window, _| source(window))
                        .unwrap();
                    let plan = PaintPlan::record(record);
                    for matrix in affine() {
                        let device = TransformationMatrix {
                            rotation_scale: matrix.rotation_scale,
                            translation: [
                                matrix.translation[0] * density,
                                matrix.translation[1] * density,
                            ],
                        };
                        let (_, live) = window
                            .record_paint(extent, cx, |window, _| {
                                window.with_transformation_masked(device, source)
                            })
                            .unwrap();
                        let (_, replayed) = window
                            .record_paint(extent, cx, |window, _| {
                                window
                                    .with_paint_presentation(matrix, |window| {
                                        window.paint_plan(&plan, placement(0., 0., extent)).unwrap();
                                    })
                                    .unwrap();
                            })
                            .unwrap();
                        let (a, b) = (live.scene(), replayed.scene());
                        assert_eq!(a.quads.len(), b.quads.len());
                        for (a, b) in a.quads.iter().zip(&b.quads) {
                            assert_eq!(a.transformation, b.transformation);
                            assert_eq!(a.bounds, b.bounds);
                            // The same visible pixels: the replay may also clip
                            // to the presented record extent, which covers them.
                            let shown = placed(a.bounds, &a.transformation);
                            assert_eq!(
                                a.content_mask.bounds.intersect(&shown),
                                b.content_mask.bounds.intersect(&shown)
                            );
                        }
                        assert_eq!(a.paths.len(), b.paths.len());
                        for (a, b) in a.paths.iter().zip(&b.paths) {
                            assert_eq!(a.bounds, b.bounds);
                            assert_eq!(
                                a.content_mask.bounds.intersect(&a.bounds),
                                b.content_mask.bounds.intersect(&b.bounds)
                            );
                        }
                        assert_eq!(a.underlines.len(), b.underlines.len());
                        for (a, b) in a.underlines.iter().zip(&b.underlines) {
                            assert_eq!(a.bounds, b.bounds);
                            assert_eq!(
                                a.content_mask.bounds.intersect(&a.bounds),
                                b.content_mask.bounds.intersect(&b.bounds)
                            );
                        }
                    }
                },
            )
        });
    }
}

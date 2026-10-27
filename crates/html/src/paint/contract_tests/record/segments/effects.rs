//! Маска и transform принадлежат единственному экземпляру scope каждого сегмента.

use super::*;
use std::sync::Arc;

#[test]
fn ancestor_transform_is_applied_once_before_independent_segment_placement() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let extent = rect(0., 0., 90., 90.);
                let transform = gpui::TransformationMatrix {
                    translation: [7., 9.],
                    ..gpui::TransformationMatrix::unit()
                };
                let (_, records) = window
                    .record_paint_segments(
                        extent,
                        Rc::from([route(1, 100.), route(2, 200.)]),
                        cx,
                        |window, _| {
                            window.with_transformation(transform, |window| {
                                window.paint_group(
                                    extent,
                                    Corners::default(),
                                    0.,
                                    0.5,
                                    0,
                                    &[],
                                    None,
                                    None,
                                    |window| {
                                        quad(window, rect(10., 10., 10., 10.));
                                        window
                                            .with_paint_segments(
                                                Rc::from([route(2, 220.)]),
                                                |window| {
                                                    quad(window, rect(20., 20., 10., 10.));
                                                },
                                            )
                                            .unwrap();
                                    },
                                );
                            })
                        },
                    )
                    .unwrap();
                for (id, expected) in [(1, vec![107.]), (2, vec![207., 227.])] {
                    let scene = records.get(PaintSegmentId(id)).unwrap().scene();
                    assert_eq!(scene.groups.len(), 1);
                    assert_eq!(scene.surfaces.len(), 1);
                    let quads = &scene.groups[0].scene.quads;
                    assert_eq!(quads.len(), expected.len());
                    for (quad, x) in quads.iter().zip(expected) {
                        assert_eq!(quad.transformation.translation, [x, 9.]);
                    }
                }
                assert_eq!(
                    window.current_transformation(),
                    gpui::TransformationMatrix::unit()
                );
            },
        )
    });
}

#[test]
fn mask_is_shared_by_normal_and_independently_moved_oof_leaves() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let extent = rect(0., 0., 90., 90.);
                let mask = Arc::new(gpui::RenderImage::new(vec![image::Frame::new(
                    image::RgbaImage::from_pixel(2, 2, image::Rgba([0, 0, 0, 128])),
                )]));
                let (_, records) = window
                    .record_paint_segments(
                        extent,
                        Rc::from([route(1, 100.), route(2, 200.)]),
                        cx,
                        |window, _| {
                            window.paint_group(
                                extent,
                                Corners::default(),
                                0.,
                                0.5,
                                0,
                                &[],
                                Some((mask.clone(), rect(5., 6., 70., 70.), 3)),
                                Some([5., 6., 70., 70.]),
                                |window| {
                                    quad(window, rect(10., 10., 10., 10.));
                                    window
                                        .with_paint_segments(Rc::from([route(2, 220.)]), |window| {
                                            quad(window, rect(20., 20., 10., 10.));
                                        })
                                        .unwrap();
                                },
                            );
                        },
                    )
                    .unwrap();
                for (id, x, count) in [(1, 100., 1), (2, 200., 2)] {
                    let scene = records.get(PaintSegmentId(id)).unwrap().scene();
                    assert_eq!(scene.groups.len(), 1);
                    assert_eq!(scene.surfaces.len(), 1);
                    assert_eq!(scene.surfaces[0].opacity, 0.5);
                    let group = &scene.groups[0];
                    assert!(Arc::ptr_eq(group.mask.as_ref().unwrap(), &mask));
                    assert_eq!(group.mask_bounds.origin.x.0, x + 5.);
                    assert_eq!(group.mask_clip, Some([x + 5., 6., 70., 70.]));
                    assert_eq!(group.mask_once, 3);
                    assert_eq!(group.scene.quads.len(), count);
                }
            },
        )
    });
}

//! Scope присутствует в сегменте потомка даже без собственного normal-фрагмента.

use super::*;
use gpui::ContentMask;
use std::sync::Arc;

#[test]
fn ancestor_without_own_fragment_keeps_its_scope_placement_for_oof() {
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
                            window
                                .with_paint_segments(Rc::from([route(1, 100.)]), |window| {
                                    window.with_content_mask(
                                        Some(ContentMask {
                                            bounds: rect(10., 0., 50., 90.),
                                        }),
                                        |window| {
                                            window.paint_group(
                                                extent,
                                                Corners::default(),
                                                3.,
                                                0.5,
                                                0,
                                                &[],
                                                Some((mask.clone(), rect(5., 6., 70., 70.), 0)),
                                                None,
                                                |window| {
                                                    quad(window, rect(15., 10., 10., 10.));
                                                    window
                                                        .with_paint_segments(
                                                            Rc::from([route(2, 220.)]),
                                                            |window| {
                                                                quad(
                                                                    window,
                                                                    rect(20., 20., 10., 10.),
                                                                );
                                                            },
                                                        )
                                                        .unwrap();
                                                },
                                            );
                                        },
                                    );
                                })
                                .unwrap();
                        },
                    )
                    .unwrap();
                for (id, x, leaf_x) in [(1, 100., 100.), (2, 200., 220.)] {
                    let scene = records.get(PaintSegmentId(id)).unwrap().scene();
                    assert_eq!(scene.groups.len(), 1);
                    assert_eq!(scene.surfaces.len(), 1);
                    assert_eq!(scene.surfaces[0].opacity, 0.5);
                    assert_eq!(scene.groups[0].mask_bounds.origin.x.0, x + 5.);
                    assert_eq!(scene.surfaces[0].content_mask.bounds.origin.x.0, x + 10.);
                    assert_eq!(scene.surfaces[0].content_mask.bounds.right().0, x + 60.);
                    assert_eq!(scene.groups[0].scene.quads.len(), 1);
                    assert_eq!(
                        scene.groups[0].scene.quads[0].transformation.translation[0],
                        leaf_x
                    );
                }
            },
        )
    });
}

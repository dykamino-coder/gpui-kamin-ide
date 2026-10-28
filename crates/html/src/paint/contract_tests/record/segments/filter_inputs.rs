//! Overflow внутри эффекта ограничивает вход, сохраняя выход фильтра за клипом.

use super::*;
use gpui::ContentMask;

#[test]
fn overflow_inside_filter_clips_inputs_without_clipping_the_shared_output() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let extent = rect(0., 0., 100., 100.);
                let (_, records) = window
                    .record_paint_segments(extent, Rc::from([route(1, 100.)]), cx, |window, _| {
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
                                let outside = window.paint_segment_clips().unwrap();
                                window.with_content_mask(
                                    Some(ContentMask {
                                        bounds: rect(10., 0., 20., 100.),
                                    }),
                                    |window| {
                                        quad(window, rect(15., 5., 20., 10.));
                                        window
                                            .with_paint_segments(
                                                Rc::from([route(1, 120.)]),
                                                |window| {
                                                    window
                                                        .with_paint_segment_clips(
                                                            &outside,
                                                            |window| {
                                                                quad(
                                                                    window,
                                                                    rect(45., 20., 10., 10.),
                                                                );
                                                            },
                                                        )
                                                        .unwrap();
                                                },
                                            )
                                            .unwrap();
                                    },
                                );
                            },
                        );
                    })
                    .unwrap();
                let scene = records.get(PaintSegmentId(1)).unwrap().scene();
                assert_eq!(scene.groups.len(), 1);
                assert_eq!(scene.surfaces.len(), 1);
                assert_eq!(scene.surfaces[0].opacity, 0.5);
                let input = &scene.groups[0].scene.quads;
                assert_eq!(input.len(), 2);
                assert_eq!(input[0].content_mask.bounds.origin.x.0, 110.);
                assert_eq!(input[0].content_mask.bounds.right().0, 130.);
                assert!(input[1].content_mask.bounds.right().0 >= 175.);
                assert!(scene.surfaces[0].content_mask.bounds.right().0 >= 175.);
            },
        )
    });
}

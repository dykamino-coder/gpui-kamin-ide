//! Baseline capture: sequence идентифицирует readback, offscreen сохраняет хозяина.

use super::draw;
use gpui::{Bounds, ContentMask, TransformationMatrix, canvas, frame_capture, point, px, size};
use std::rc::Rc;

#[test]
fn capture_sequence_distinguishes_first_latest_and_completion() {
    // Текущий capture API process-wide и допускает одного хозяина. Этот тест
    // не приписывает sequence отсутствующую identity окна или документа.
    let before = frame_capture::sequence();
    let mut completed = frame_capture::after(before);
    frame_capture::arm_first();
    let first_pixels = vec![1, 2, 3, 255, 4, 5, 6, 255];
    frame_capture::store(2, 1, first_pixels.clone());
    let first_seq = frame_capture::sequence();
    assert!(first_seq > before);
    assert_eq!(completed.try_recv().unwrap(), Some(first_seq));

    let latest_pixels = vec![7, 8, 9, 255, 10, 11, 12, 255];
    frame_capture::store(1, 2, latest_pixels.clone());
    let latest = frame_capture::latest().unwrap();
    assert!(latest.seq > first_seq);
    assert_eq!(
        (latest.width, latest.height, latest.bgra),
        (1, 2, latest_pixels)
    );
    let first = frame_capture::take_first().unwrap();
    assert_eq!(
        (first.seq, first.width, first.height, first.bgra),
        (first_seq, 2, 1, first_pixels)
    );
    assert!(frame_capture::take_first().is_none());
    assert_eq!(
        frame_capture::after(first_seq).try_recv().unwrap(),
        Some(latest.seq)
    );

    frame_capture::arm_first();
    assert!(frame_capture::take_first().is_none());
    let mut next = frame_capture::after(latest.seq);
    assert_eq!(next.try_recv().unwrap(), None);
    frame_capture::store(1, 1, vec![13, 14, 15, 255]);
    let rearmed = frame_capture::take_first().unwrap();
    assert!(rearmed.seq > latest.seq);
    assert_eq!(next.try_recv().unwrap(), Some(rearmed.seq));
    assert_eq!(rearmed.bgra, [13, 14, 15, 255]);
}

#[test]
fn offscreen_capture_keeps_host_identity_and_restores_outer_geometry_on_backend_error() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, _| {
                let host = Rc::new(17u32);
                let transform = TransformationMatrix {
                    translation: [5.25, 7.75],
                    ..TransformationMatrix::unit()
                };
                let mask = ContentMask {
                    bounds: Bounds::new(point(px(4.), px(6.)), size(px(40.), px(50.))),
                };
                window.with_render_context(Some(host.clone()), |window| {
                    window.with_transformation(transform, |window| {
                        window.with_content_mask(Some(mask), |window| {
                            let outer_mask = window.content_mask();
                            let outer_scale = window.scale_factor();
                            for local_size in [size(px(8.), px(9.)), size(px(10.), px(11.))] {
                                let mut painted = false;
                                let result = window.paint_scene_to_png(local_size, 2., |window| {
                                    painted = true;
                                    assert!(Rc::ptr_eq(
                                        &window.render_context::<u32>().unwrap(),
                                        &host
                                    ));
                                    assert_eq!(
                                        window.current_transformation(),
                                        TransformationMatrix::unit()
                                    );
                                    assert_eq!(
                                        window.content_mask().bounds,
                                        Bounds::new(point(px(0.), px(0.)), local_size)
                                    );
                                    assert_eq!(window.scale_factor(), outer_scale * 2.);
                                });
                                // TestPlatform не реализует GPU rasterize_scene. Ошибка приходит
                                // после paint closure: внешний scene/state уже должны быть возвращены.
                                assert!(painted);
                                assert!(result.is_err());
                                assert_eq!(window.current_transformation(), transform);
                                assert_eq!(window.content_mask(), outer_mask);
                                assert_eq!(window.scale_factor(), outer_scale);
                                assert!(Rc::ptr_eq(
                                    &window.render_context::<u32>().unwrap(),
                                    &host
                                ));
                            }
                        });
                    });
                });
                assert!(window.render_context::<u32>().is_none());
            },
        )
    });
}

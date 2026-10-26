//! Production offscreen capture сохраняет хозяина и восстанавливает геометрию.

use super::draw;
use gpui::{Bounds, ContentMask, TransformationMatrix, canvas, point, px, size};
use std::rc::Rc;

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

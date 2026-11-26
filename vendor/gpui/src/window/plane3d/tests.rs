use super::*;
use crate::{TestAppContext, canvas, fill, prelude::*, px, red};

/// A 3D context of two crossing planes: a turned one painting nothing (an
/// empty `preserve-3d` container) and a flat painted one buffered under an
/// identity homography.
struct Crossing;

const SIDE: f32 = 100.0;

fn spec(full: [[f32; 4]; 4]) -> PlaneSpec {
    PlaneSpec {
        raster: TransformationMatrix::unit(),
        projection: Some([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]),
        full,
        outer: TransformationMatrix::unit(),
        rect: (0.0, 0.0, SIDE, SIDE),
        masked: false,
    }
}

impl Render for Crossing {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        canvas(
            |_, _, _| {},
            |_, _, window, _| {
                window.paint_plane_context(|window| {
                    // Turned about the vertical line x = 50 (z = x - 50, not
                    // edge-on): it crosses the flat plane there.
                    let turned = [[0.5, 0.0, 0.0, 25.0], [0.0, 1.0, 0.0, 0.0], [1.0, 0.0, 0.0, -50.0], [0.0, 0.0, 0.0, 1.0]];
                    window.paint_plane(spec(turned), |_| {});
                    let flat = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
                    window.paint_plane(spec(flat), |window| {
                        window.paint_quad(fill(Bounds::new(point(px(0.), px(0.)), size(px(SIDE), px(SIDE))), red()));
                    });
                });
            },
        )
        .size(px(200.))
    }
}

/// A plane that paints nothing gets no buffer and no composite, and does not
/// split the plane it crosses: the painted plane stays one piece.
#[crate::test]
fn a_plane_painting_nothing_neither_composites_nor_splits_another(cx: &mut TestAppContext) {
    let window = cx.add_window(|_, _| Crossing);
    cx.run_until_parked();
    window
        .update(cx, |_, window, _| {
            let scene = &window.rendered_frame.scene;
            assert_eq!(scene.groups.len(), 1, "one buffer: the painted plane's");
            assert_eq!(scene.surfaces.len(), 1, "one composite of one piece");
            assert_eq!(scene.surfaces[0].fragment, 0);
        })
        .unwrap();
}

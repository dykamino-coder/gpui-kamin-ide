//! Размещение clip своего scope одинаково на обеих осях и при дробном масштабе.

use super::*;
use gpui::ContentMask;

#[test]
fn parent_clip_uses_its_own_two_axis_placement_at_each_raster_scale() {
    for scale in [1., 1.25, 1.5, 2.] {
        super::super::super::draw_at_scale(Some(scale), move || {
            canvas(
                |_, _, _| (),
                move |_, (), window, cx| {
                    let parent = PaintSegment {
                        id: PaintSegmentId(2),
                        source: point(px(5.), px(7.)),
                        destination: point(px(105.), px(207.)),
                    };
                    let child = PaintSegment {
                        destination: point(px(125.), px(217.)),
                        ..parent
                    };
                    let (_, records) = window
                        .record_paint_segments(
                            rect(0., 0., 90., 90.),
                            Rc::from([parent]),
                            cx,
                            |window, _| {
                                window.with_content_mask(
                                    Some(ContentMask {
                                        bounds: rect(10., 20., 30., 40.),
                                    }),
                                    |window| {
                                        window
                                            .with_paint_segments(Rc::from([child]), |window| {
                                                quad(window, rect(15., 25., 20., 10.));
                                            })
                                            .unwrap()
                                    },
                                );
                            },
                        )
                        .unwrap();
                    let scene = records.get(PaintSegmentId(2)).unwrap().scene();
                    assert_eq!(scene.quads.len(), 1);
                    let quad = &scene.quads[0];
                    assert_eq!(
                        quad.transformation.translation,
                        [120. * scale, 210. * scale]
                    );
                    assert_eq!(
                        quad.content_mask.bounds,
                        rect(110., 220., 30., 40.).scale(scale)
                    );
                },
            )
        });
    }
}

//! Поднятая подложка остаётся в сегменте вложенного источника.

use super::*;

#[test]
fn hoisted_backdrop_keeps_its_independent_segment_through_enclosing_groups() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let extent = rect(0., 0., 90., 90.);
                let (_, records) = window
                    .record_paint_segments(
                        extent,
                        Rc::from([route(1, 0.), route(2, 100.)]),
                        cx,
                        |window, _| {
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
                                    window
                                        .with_paint_segments(Rc::from([route(2, 120.)]), |window| {
                                            window.paint_backdrop_filter(
                                                rect(4., 5., 12., 13.),
                                                Corners::default(),
                                                2.,
                                                [0.; 20],
                                                true,
                                            );
                                            quad(window, rect(4., 5., 12., 13.));
                                        })
                                        .unwrap();
                                },
                            );
                        },
                    )
                    .unwrap();
                for (id, count) in [(1, 0), (2, 1)] {
                    let plan =
                        gpui::PaintPlan::record(records.get(PaintSegmentId(id)).unwrap().clone());
                    let (_, output) = window
                        .record_paint(rect(0., 0., 250., 100.), cx, |window, _| {
                            window
                                .paint_plan(&plan, placement(0., 0., rect(0., 0., 250., 100.)))
                                .unwrap();
                        })
                        .unwrap();
                    // record_paint сохраняет подложки для подъёма до кадра,
                    // а не вставляет их в промежуточный буфер группы.
                    let backdrops = output.hoisted_backdrops();
                    assert_eq!(backdrops.len(), count);
                    if count > 0 {
                        assert_eq!(backdrops[0].bounds.origin.x.0, 124.);
                    }
                }
            },
        )
    });
}

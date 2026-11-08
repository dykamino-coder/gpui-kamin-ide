//! Соседние raster captures одного прохода делят окружение; оно принадлежит проходу.

use super::draw_at_scale;
use gpui::{RasterCaptures, TransformationMatrix, canvas};

#[test]
fn sibling_raster_captures_share_one_environment_and_replay_their_own_ids() {
    draw_at_scale(Some(1.0), || {
        canvas(
            |_, _, _| (),
            |_, (), window, _| {
                let captures = RasterCaptures::default();
                let id = |window: &mut gpui::Window| {
                    window.with_global_id("probe".into(), |id, _| format!("{id:?}"))
                };
                let capture = |window: &mut gpui::Window, name: &'static str| {
                    window.with_global_id(name.into(), |_, window| {
                        (window.raster_paint_ctx(&captures), id(window))
                    })
                };
                let (a, a_id) = capture(window, "a");
                let (b, b_id) = capture(window, "b");
                assert!(a.shares_environment(&b));
                assert_ne!(a_id, b_id);
                let moved = TransformationMatrix {
                    translation: [1.0, 0.0],
                    ..TransformationMatrix::unit()
                };
                let (c, _) = window.with_transformation(moved, |window| capture(window, "c"));
                assert!(!c.shares_environment(&a));
                let (d, _) = capture(window, "d");
                assert!(!d.shares_environment(&a));
                let outside = id(window);
                for (ctx, expected) in [(b, b_id), (a, a_id)] {
                    let replayed = window
                        .with_raster_paint_ctx(ctx, &captures, |window| id(window))
                        .unwrap();
                    assert_eq!(replayed, expected);
                    assert_eq!(id(window), outside);
                }
                // The pass owns the shared environment: once it ends, a capture starts anew,
                // and a capture of another pass shares nothing with it.
                let (e, _) = capture(window, "e");
                let (f, _) = capture(window, "f");
                assert!(e.shares_environment(&f));
                captures.clear();
                let (g, _) = capture(window, "g");
                assert!(!g.shares_environment(&f));
                let other = RasterCaptures::default();
                let h = window.with_global_id("h".into(), |_, w| w.raster_paint_ctx(&other));
                assert!(!h.shares_environment(&g));
            },
        )
    });
}

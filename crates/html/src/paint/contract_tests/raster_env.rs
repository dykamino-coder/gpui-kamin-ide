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

#[test]
fn shared_raster_replay_restores_ids_after_suspension_and_cancellation() {
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    draw_at_scale(Some(1.0), || {
        canvas(
            |_, _, _| (),
            |_, (), window, _| {
                let captures = RasterCaptures::default();
                let id = |window: &mut gpui::Window| {
                    window.with_global_id("probe".into(), |id, _| format!("{id:?}"))
                };
                let (a, expected) = window.with_global_id("a".into(), |_, window| {
                    (window.raster_paint_ctx(&captures), id(window))
                });
                let b = window
                    .with_global_id("b".into(), |_, window| window.raster_paint_ctx(&captures));
                assert!(a.shares_environment(&b));
                let outside = id(window);
                let mut cx = Context::from_waker(Waker::noop());
                {
                    let mut resumed = false;
                    let replay = window.with_raster_paint_ctx_async(a, &captures, async |window| {
                        assert_eq!(id(window), expected);
                        std::future::poll_fn(|_| {
                            if std::mem::replace(&mut resumed, true) {
                                Poll::Ready(())
                            } else {
                                Poll::Pending
                            }
                        })
                        .await;
                        assert_eq!(id(window), expected);
                    });
                    let mut replay = std::pin::pin!(replay);
                    assert!(replay.as_mut().poll(&mut cx).is_pending());
                    assert!(matches!(replay.as_mut().poll(&mut cx), Poll::Ready(Ok(()))));
                }
                assert_eq!(id(window), outside);
                {
                    let replay = window.with_raster_paint_ctx_async(b, &captures, async |_| {
                        std::future::pending::<()>().await;
                    });
                    let mut replay = std::pin::pin!(replay);
                    assert!(replay.as_mut().poll(&mut cx).is_pending());
                }
                assert_eq!(id(window), outside);
                let next = window.raster_paint_ctx(&captures);
                assert_eq!(
                    window.with_raster_paint_ctx(next, &captures, id).unwrap(),
                    outside
                );
                captures.clear();
            },
        )
    });
}

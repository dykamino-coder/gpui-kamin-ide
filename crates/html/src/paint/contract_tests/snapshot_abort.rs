//! Прерванный production draw закрывает токены и восстанавливает следующий проход.

use gpui::{RasterFrame, canvas, point, px};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[test]
fn aborted_draw_closes_tokens_and_next_draw_has_clean_state() {
    type Saved =
        Rc<std::cell::RefCell<Option<(gpui::PaintState, gpui::PaintPassId, gpui::PaintScopeId)>>>;
    struct Probe {
        saved: Saved,
        initial: RasterFrame,
        abort: bool,
    }
    impl gpui::Render for Probe {
        fn render(
            &mut self,
            _: &mut gpui::Window,
            _: &mut gpui::Context<Self>,
        ) -> impl gpui::IntoElement {
            let saved = self.saved.clone();
            let initial = self.initial.clone();
            let abort = std::mem::replace(&mut self.abort, false);
            canvas(
                |_, _, _| (),
                move |_, (), window, _| {
                    if abort {
                        *saved.borrow_mut() = Some((
                            window.snapshot_paint_state(),
                            window.paint_pass_id(),
                            window.paint_scope_id(),
                        ));
                        window.replace_glyph_offset(point(px(5.25), px(7.75)));
                        window.with_element_namespace("aborted", |_| panic!("abort draw"));
                    } else {
                        let (snapshot, pass, scope) = saved.borrow_mut().take().unwrap();
                        assert!(!pass.is_live());
                        assert!(!scope.is_live());
                        assert!(window.paint_pass_id().is_live());
                        assert_eq!(window.raster_frame().clone(), initial);
                        assert_eq!(
                            window.with_paint_state(snapshot, |_| panic!("stale replay")),
                            Err::<(), _>(gpui::PaintError::ExpiredPass)
                        );
                    }
                },
            )
        }
    }
    let mut cx = gpui::TestAppContext::single();
    let mut visual = cx.add_empty_window().clone();
    let saved = Rc::new(std::cell::RefCell::new(None));
    visual.update(|window, cx| {
        let initial = window.raster_frame().clone();
        window.replace_root(cx, |_, _| Probe {
            saved: saved.clone(),
            initial,
            abort: true,
        });
        // Keep the application's window lease outside the unwind boundary.
        assert!(catch_unwind(AssertUnwindSafe(|| window.draw(cx))).is_err());
        window.draw(cx).clear(cx);
    });
    assert!(saved.borrow().is_none());
    cx.quit();
}

#[test]
fn aborted_prepaint_does_not_leave_layout_input_collection_open() {
    struct Abort(Rc<std::cell::Cell<usize>>);
    impl gpui::Render for Abort {
        fn render(
            &mut self,
            _: &mut gpui::Window,
            _: &mut gpui::Context<Self>,
        ) -> impl gpui::IntoElement {
            let attempts = self.0.clone();
            canvas(
                move |_, window, _| {
                    let attempt = attempts.get();
                    attempts.set(attempt + 1);
                    if attempt == 0 {
                        assert!(
                            window.update_layout_input(gpui::LayoutInputId::default(), || true)
                        );
                        panic!("abort prepaint");
                    }
                },
                |_, (), _, _| {},
            )
        }
    }
    let mut cx = gpui::TestAppContext::single();
    let mut visual = cx.add_empty_window().clone();
    let attempts = Rc::new(std::cell::Cell::new(0));
    visual.update(|window, cx| {
        window.replace_root(cx, |_, _| Abort(attempts.clone()));
        // Keep the application's window lease outside the unwind boundary.
        assert!(catch_unwind(AssertUnwindSafe(|| window.draw(cx))).is_err());
    });
    let called = Rc::new(std::cell::Cell::new(false));
    let updater_called = called.clone();
    visual.draw(
        point(px(0.), px(0.)),
        gpui::size(px(20.), px(20.)),
        |_, _| {
            canvas(
                move |_, window, _| {
                    // No production redraw may reset the state before this observation.
                    assert_eq!(attempts.get(), 1);
                    assert!(
                        !window.update_layout_input(gpui::LayoutInputId::default(), || {
                            updater_called.set(true);
                            true
                        })
                    );
                },
                |_, (), _, _| {},
            )
        },
    );
    assert!(!called.get());
    cx.quit();
}

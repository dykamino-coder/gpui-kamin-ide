//! Проверка живой области снимка без исполнения отклонённого paint callback.

use super::draw;
use gpui::{AppContext, Bounds, Corners, PaintError, PaintState, Window, canvas, point, px, size};
use std::{cell::RefCell, rc::Rc};

fn group<R>(window: &mut Window, f: impl FnOnce(&mut Window) -> R) -> R {
    window.paint_group(
        Bounds::new(point(px(0.), px(0.)), size(px(20.), px(20.))),
        Corners::default(),
        0.,
        0.5,
        0,
        &[],
        None,
        None,
        f,
    )
}

fn rejected(window: &mut Window, snapshot: PaintState, error: PaintError) {
    let before = window.raster_frame().clone();
    let result = window.with_paint_state(snapshot, |_| panic!("invalid replay ran"));
    assert_eq!(result, Err::<(), _>(error));
    assert_eq!(window.raster_frame().clone(), before);
}

#[test]
fn composition_identity_rejects_parent_sibling_and_closed_group() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, _| {
                let parent = window.snapshot_paint_state();
                let parent_scope = window.paint_scope_id();
                let (closed, scope) = group(window, |window| {
                    let scope = window.paint_scope_id();
                    assert!(scope.is_live());
                    rejected(window, parent.clone(), PaintError::WrongScope);
                    let inside = window.snapshot_paint_state();
                    window.with_paint_state(inside.clone(), |_| ()).unwrap();
                    group(window, |window| {
                        rejected(window, inside.clone(), PaintError::WrongScope);
                    });
                    window.with_paint_state(inside.clone(), |_| ()).unwrap();
                    (inside, scope)
                });
                assert!(!scope.is_live());
                assert!(parent_scope.is_live());
                rejected(window, closed.clone(), PaintError::ExpiredScope);
                group(window, |window| {
                    rejected(window, closed, PaintError::ExpiredScope);
                });
                window.with_paint_state(parent, |_| ()).unwrap();
            },
        )
    });
}

#[test]
fn offscreen_capture_has_a_separate_live_scope() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, _| {
                let outer = window.snapshot_paint_state();
                let mut captured = None;
                let result = window.paint_scene_to_png(size(px(8.), px(9.)), 1.5, |window| {
                    rejected(window, outer.clone(), PaintError::WrongScope);
                    let inner = window.snapshot_paint_state();
                    window.with_paint_state(inner.clone(), |_| ()).unwrap();
                    captured = Some(inner);
                });
                assert!(result.is_err()); // Fake backend has no GPU readback.
                rejected(window, captured.unwrap(), PaintError::ExpiredScope);
                window.with_paint_state(outer, |_| ()).unwrap();
            },
        )
    });
}

#[test]
fn snapshot_clone_does_not_extend_pass_or_window_identity() {
    let mut cx = gpui::TestAppContext::single();
    let mut first = cx.add_empty_window().clone();
    let second = cx
        .add_empty_window()
        .update(|window, _| window.window_handle());
    let saved = Rc::new(RefCell::new(None));
    let output = saved.clone();
    first.draw(point(px(0.), px(0.)), size(px(40.), px(40.)), |_, _| {
        canvas(
            |_, _, _| (),
            move |_, (), window, cx| {
                let snapshot = window.snapshot_paint_state();
                // A distinct window rejects even while the source pass is live.
                cx.update_window(second, |_, other, _| {
                    rejected(other, snapshot.clone(), PaintError::WrongWindow);
                })
                .unwrap();
                *output.borrow_mut() = Some((snapshot, window.paint_pass_id()));
            },
        )
    });
    let (snapshot, pass) = saved.borrow_mut().take().unwrap();
    assert!(!pass.is_live());
    first.draw(point(px(0.), px(0.)), size(px(40.), px(40.)), |_, _| {
        canvas(
            |_, _, _| (),
            move |_, (), window, _| {
                assert!(window.paint_pass_id().is_live());
                rejected(window, snapshot, PaintError::ExpiredPass);
            },
        )
    });
    cx.quit();
}

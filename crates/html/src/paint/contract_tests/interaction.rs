//! Отложенный paint сохраняет prepaint hitbox identity и порядок scroll callbacks.

use super::collect;
use crate::paint::coordinator::PaintLast;
use gpui::{
    Bounds, Hitbox, HitboxBehavior, IntoElement, ScrollDelta, ScrollWheelEvent, TestAppContext,
    canvas, point, px, size,
};
use std::{cell::RefCell, rc::Rc};

#[test]
fn deferred_paint_keeps_hitboxes_and_scroll_listener_priority() {
    let registered = Rc::new(RefCell::new(Vec::new()));
    let painted = Rc::new(RefCell::new(Vec::new()));
    let scrolled = Rc::new(RefCell::new(Vec::new()));
    let mut app = TestAppContext::single();
    let cx = app.add_empty_window();
    cx.draw(point(px(0.), px(0.)), size(px(100.), px(100.)), |_, _| {
        let leaf = |name| {
            let registered = registered.clone();
            let expected = registered.clone();
            let painted = painted.clone();
            let scrolled = scrolled.clone();
            canvas(
                move |_, window, _| {
                    let hitbox = window.insert_hitbox(
                        Bounds::new(point(px(0.), px(0.)), size(px(50.), px(50.))),
                        HitboxBehavior::Normal,
                    );
                    registered.borrow_mut().push((name, hitbox.clone()));
                    hitbox
                },
                move |_, hitbox: Hitbox, window, _| {
                    let expected = expected.borrow();
                    let original = &expected.iter().find(|(n, _)| *n == name).unwrap().1;
                    assert_eq!(hitbox.id, original.id);
                    assert_eq!(hitbox.bounds, original.bounds);
                    painted.borrow_mut().push(name);
                    let scrolled = scrolled.clone();
                    window.on_mouse_event::<ScrollWheelEvent>(move |_, phase, window, _| {
                        if phase.bubble() && hitbox.should_handle_scroll(window) {
                            scrolled.borrow_mut().push(name);
                        }
                    });
                },
            )
            .into_any_element()
        };
        crate::hooks::paint_root(
            collect(vec![
                PaintLast::new(leaf("positioned")).into_any_element(),
                leaf("flow"),
            ])
            .into_any_element(),
        )
    });
    assert_eq!(
        registered
            .borrow()
            .iter()
            .map(|(n, _)| *n)
            .collect::<Vec<_>>(),
        ["positioned", "flow"]
    );
    assert_eq!(*painted.borrow(), ["flow", "positioned"]);
    cx.simulate_event(ScrollWheelEvent {
        position: point(px(1.), px(1.)),
        delta: ScrollDelta::Pixels(point(px(0.), px(-1.))),
        ..Default::default()
    });
    assert_eq!(*scrolled.borrow(), ["positioned", "flow"]);
    app.quit();
}

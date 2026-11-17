//! Проверки освобождения арены после завершения отложенного позиционирования.

use super::*;
use gpui::{IntoElement, RetainedLayout, Styled, size};
use std::cell::Cell;

struct BorrowOnDrop(Weak<RefCell<Work>>, Rc<Cell<bool>>);

impl Drop for BorrowOnDrop {
    fn drop(&mut self) {
        if let Some(work) = self.0.upgrade() {
            assert!(work.borrow().destinations.is_empty());
            self.1.set(true);
        }
    }
}

fn place(_: super::super::super::SpotCell, child: AnyElement) -> AnyElement {
    child
}

#[gpui::test]
fn completed_pending_releases_the_layout_arena_owned_by_its_measure_callback(
    cx: &mut gpui::TestAppContext,
) {
    cx.add_empty_window().draw(
        point(px(0.), px(0.)),
        size(px(100.), px(100.)),
        |window, app| {
            let pending = Pending::default();
            let queue = OofQueue::with_pending(pending.clone());
            let output = Rc::default();
            let marker = Rc::new(());
            let weak = Rc::downgrade(&marker);
            let dropped = Rc::new(Cell::new(false));
            let on_drop = BorrowOnDrop(Rc::downgrade(&pending.0), dropped.clone());
            let mut arena = RetainedLayout::default();
            window.with_retained_layout(&mut arena, |window| {
                let captured = pending.clone();
                window.request_measured_layout(Default::default(), move |_, _, _, _| {
                    let _ = (&captured, &marker, &on_drop);
                    size(px(1.), px(1.))
                });
                pending.register(
                    &queue,
                    Bounds::new(point(px(0.), px(0.)), size(px(100.), px(100.))),
                    place,
                    pending.context(window),
                    &output,
                );
            });
            drop(arena);
            // The registered context still owns the arena; its measure callback
            // points back at Pending. Finishing must break this real Rc cycle.
            assert!(weak.upgrade().is_some());
            pending.finish(window, app);
            assert!(weak.upgrade().is_none());
            assert!(dropped.get());
            gpui::div()
        },
    );
}

#[gpui::test]
fn finishing_keeps_empty_destinations_until_late_children_have_been_prepainted(
    cx: &mut gpui::TestAppContext,
) {
    cx.add_empty_window().draw(
        point(px(0.), px(0.)),
        size(px(100.), px(100.)),
        |window, app| {
            let pending = Pending::default();
            let queue = OofQueue::with_pending(pending.clone());
            let output = Rc::new(RefCell::new(Vec::new()));
            let context = pending.context(window);
            let bounds = Bounds::new(point(px(0.), px(0.)), size(px(100.), px(100.)));
            pending.register(&queue, bounds, place, context.clone(), &output);
            let builds = Rc::new(Cell::new(0));
            let observed = builds.clone();
            let late_queue = queue.clone();
            pending.push(
                (
                    Default::default(),
                    OofKid::Deferred(
                        Box::new(move |_, _, _| {
                            late_queue.push_kid(
                                Default::default(),
                                OofKid::Deferred(
                                    Box::new(move |_, _, _| {
                                        builds.set(builds.get() + 1);
                                        gpui::div().w(px(10.)).h(px(5.)).into_any_element()
                                    }),
                                    None,
                                ),
                            );
                            gpui::div().into_any_element()
                        }),
                        None,
                    ),
                ),
                bounds,
                place,
                context,
                output.clone(),
            );
            pending.finish(window, app);
            assert_eq!(observed.get(), 1);
            assert_eq!(output.borrow().len(), 2);
            pending.finish(window, app);
            assert_eq!(output.borrow().len(), 2);
            gpui::div()
        },
    );
}

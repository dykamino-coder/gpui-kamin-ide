//! Replay переносит typed context по идентичности; unwind после abort освобождает jobs.

use super::{Events, assert_phases, collect, draw, probe};
use crate::hooks::{IntoHooked, Wrap};
use gpui::{AnyElement, App, Bounds, IntoElement, Pixels, Window, canvas, point, px};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

struct Host {
    child: AnyElement,
    value: Rc<Cell<u32>>,
}

impl Wrap for Host {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        window.with_render_context(Some(self.value.clone()), |window| {
            let previous = window.replace_glyph_offset(point(px(0.25), px(0.75)));
            // Явный прозрачный адаптер существующего consumer, без собственной группы.
            crate::hooks::paint_reopen(window, |window| self.child.paint(window, cx));
            window.replace_glyph_offset(previous);
        });
        self.value.set(2);
    }
}

#[test]
fn queued_jobs_keep_host_identity_and_inline_raster_origin() {
    let value = Rc::new(Cell::new(1u32));
    let weak = Rc::downgrade(&value);
    draw(move || {
        let leaf = |inline| {
            let expected = value.clone();
            canvas(
                |_, _, _| (),
                move |_, (), window, _| {
                    assert!(Rc::ptr_eq(
                        &window.render_context::<Cell<u32>>().unwrap(),
                        &expected
                    ));
                    assert_eq!(
                        expected.get(),
                        2,
                        "paint happens after the host scope exits"
                    );
                    if inline {
                        let glyph = window.replace_glyph_offset(point(px(0.), px(0.)));
                        assert_eq!(glyph, point(px(0.25), px(0.75)));
                        window.replace_glyph_offset(glyph);
                    }
                },
            )
            .into_any_element()
        };
        let inline = Host {
            child: crate::paint::coordinator::PaintInline::new(leaf(true)).into_any_element(),
            value: value.clone(),
        }
        .hooked();
        let positioned = Host {
            child: crate::paint::coordinator::PaintLast::new(leaf(false)).into_any_element(),
            value: value.clone(),
        }
        .hooked();
        drop(value);
        collect(vec![inline, positioned])
    });
    assert!(
        weak.upgrade().is_none(),
        "finished jobs release their captured host"
    );
}

// Единственный сильный владелец transient context после host — queued snapshot.
struct Payload {
    child: AnyElement,
    abandoned: Rc<std::cell::RefCell<std::rc::Weak<()>>>,
}
impl Wrap for Payload {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }
    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let payload = Rc::new(());
        *self.abandoned.borrow_mut() = Rc::downgrade(&payload);
        window.with_render_context(Some(payload), |window| {
            crate::hooks::paint_reopen(window, |window| self.child.paint(window, cx));
        });
    }
}

struct Abort {
    child: AnyElement,
    abandoned: Rc<std::cell::RefCell<std::rc::Weak<()>>>,
}

impl Wrap for Abort {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        assert!(catch_unwind(AssertUnwindSafe(|| self.child.paint(window, cx))).is_err());
        assert!(
            self.abandoned.borrow().upgrade().is_none(),
            "queued payload drops during unwind"
        );
    }
}

#[test]
fn aborted_collector_releases_jobs_before_a_fresh_document() {
    let value = Rc::new(Cell::new(1u32));
    let weak = Rc::downgrade(&value);
    draw(move || {
        let abandoned = Rc::default();
        let queued = Host {
            child: crate::paint::coordinator::PaintLast::new(
                canvas(
                    |_, _, _| (),
                    |_, (), _, _| {
                        panic!("abandoned job must never paint");
                    },
                )
                .into_any_element(),
            )
            .into_any_element(),
            value,
        }
        .hooked();
        let queued = Payload {
            child: queued,
            abandoned: Rc::clone(&abandoned),
        }
        .hooked();
        let fail = canvas(|_, _, _| (), |_, (), _, _| panic!("aborted traversal"));
        Abort {
            child: collect(vec![queued, fail.into_any_element()]).into_any_element(),
            abandoned,
        }
        .into_hooked()
    });
    assert!(
        weak.upgrade().is_none(),
        "unwind drops the abandoned snapshot"
    );
    let events = Events::default();
    draw(|| {
        collect(vec![
            crate::paint::coordinator::PaintLast::new(probe("fresh", &events)).into_any_element(),
        ])
    });
    assert_phases(&events, &["fresh"], &["fresh"]);
}

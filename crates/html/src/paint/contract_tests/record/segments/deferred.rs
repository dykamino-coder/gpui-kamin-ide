//! Отложенная CSS-очередь сохраняет сегмент без повторения нативных фаз.

use super::*;
use crate::hooks::{IntoHooked, Wrap};
use crate::paint::contract_tests::{Events, assert_phases, collect, probe};
use crate::paint::coordinator::PaintLast;
use gpui::{AnyElement, App, IntoElement, ParentElement, div};

struct Selected(AnyElement);
impl Wrap for Selected {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.0
    }
    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        window.with_content_mask(
            Some(gpui::ContentMask {
                bounds: rect(0., 0., 40., 90.),
            }),
            |window| {
                window
                    .with_paint_segments(Rc::from([route(2, 120.)]), |window| {
                        crate::hooks::paint_reopen(window, |window| self.0.paint(window, cx));
                    })
                    .unwrap();
            },
        );
    }
}

struct Ink(AnyElement);
impl Wrap for Ink {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.0
    }
    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        quad(window, rect(4., 5., 12., 13.));
        self.0.paint(window, cx);
    }
}

struct Capture {
    child: AnyElement,
    events: Events,
}
impl Wrap for Capture {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }
    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let (_, records) = window
            .record_paint_segments(
                rect(0., 0., 90., 90.),
                Rc::from([route(1, 0.), route(2, 100.)]),
                cx,
                |window, cx| self.child.paint(window, cx),
            )
            .unwrap();
        self.events.borrow_mut().push(("paint", "record-finished"));
        assert_eq!(
            records.get(PaintSegmentId(1)).unwrap().scene().quads.len(),
            1
        );
        let second = &records.get(PaintSegmentId(2)).unwrap().scene().quads;
        assert_eq!(second.len(), 2);
        assert_eq!(second[0].transformation.translation[0], 100.);
        assert_eq!(second[1].transformation.translation[0], 120.);
        assert_eq!(second[1].content_mask.bounds.origin.x.0, 100.);
        assert_eq!(second[1].content_mask.bounds.right().0, 140.);
    }
}

#[test]
fn deferred_positioned_paint_keeps_selection_and_consumes_native_phases_once() {
    let events = Events::default();
    draw(|| {
        let body = collect(vec![
            div()
                .child(
                    Selected(
                        PaintLast::new(Ink(probe("late", &events)).hooked()).into_any_element(),
                    )
                    .hooked(),
                )
                .child(Ink(probe("flow", &events)).hooked())
                .into_any_element(),
        ]);
        Capture {
            child: body.into_any_element(),
            events: events.clone(),
        }
        .hooked()
    });
    assert_phases(
        &events,
        &["late", "flow"],
        &["flow", "late", "record-finished"],
    );
}

//! Запись завершает CSS-очередь в своём scope, не в следующем фрагменте.

use super::super::{Events, assert_phases, collect, probe};
use super::*;
use crate::hooks::{IntoHooked, Wrap};
use crate::paint::coordinator::{PaintInline, PaintLast};
use gpui::{AnyElement, App, IntoElement, PaintPlan, ParentElement, div};

struct Capture {
    child: AnyElement,
    events: Events,
}

impl Wrap for Capture {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let extent = rect(0., 0., 90., 90.);
        let (_, record) = window
            .record_paint(extent, cx, |window, cx| {
                self.child.paint(window, cx);
            })
            .unwrap();
        self.events.borrow_mut().push(("paint", "record-finished"));
        let plan = PaintPlan::record(record);
        for x in [0., 10.] {
            window.paint_plan(&plan, placement(x, 0., extent)).unwrap();
        }
    }
}

#[test]
fn recording_finishes_inline_and_positioned_jobs_before_scope_closes() {
    let events = Events::default();
    draw(|| {
        let body = div()
            .child(PaintLast::new(probe("late", &events)).key(10))
            .child(PaintInline::new(probe("inline", &events)))
            .child(probe("flow", &events));
        collect(vec![
            Capture {
                child: body.into_any_element(),
                events: events.clone(),
            }
            .hooked(),
            probe("next-fragment", &events),
        ])
    });
    assert_phases(
        &events,
        &["late", "inline", "flow", "next-fragment"],
        &["flow", "inline", "late", "record-finished", "next-fragment"],
    );
}

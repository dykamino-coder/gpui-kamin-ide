//! Краска блока, отложенная за его вывод (`coordinator::capture`): строки
//! блока уходят в очередь кадра стопки и снимаются записью блока; всё иное,
//! ушедшее за вывод, делает запись неполной.

use super::{Events, collect, draw, probe};
use crate::hooks::{IntoHooked, Wrap};
use crate::paint::coordinator::{LineCapture, PaintInline, PaintLast, capture};
use gpui::{AnyElement, App, Bounds, IntoElement, ParentElement, Pixels, Window, div};
use std::{cell::RefCell, rc::Rc};

type Slot = Rc<RefCell<Option<Rc<LineCapture>>>>;

struct Capturing {
    child: AnyElement,
    slot: Slot,
}

impl Wrap for Capturing {
    fn paint_boundary(&self) -> bool {
        false
    }

    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let ((), deferred) = capture(window, |window| self.child.paint(window, cx));
        self.slot.replace(Some(deferred));
    }
}

fn inline(child: AnyElement) -> AnyElement {
    PaintInline::new(child).into_any_element()
}

fn positioned(name: &'static str, events: &Events) -> AnyElement {
    PaintLast::new(probe(name, events))
        .key(1)
        .into_any_element()
}

/// Draw `block` as a capturing block between two flow siblings of a stacking
/// context; the record of its deferred paint.
fn captured(events: &Events, block: impl FnOnce() -> AnyElement) -> Rc<LineCapture> {
    let slot = Slot::default();
    draw(|| {
        let block = Capturing {
            child: block(),
            slot: slot.clone(),
        };
        collect(vec![
            probe("before", events),
            block.hooked(),
            probe("after", events),
        ])
    });
    slot.take().expect("the block painted")
}

#[test]
fn lines_of_a_block_are_captured_where_the_stacking_context_paints_them() {
    let events = Events::default();
    let deferred = captured(&events, || {
        div()
            .child(probe("flow", &events))
            .child(inline(probe("line", &events)))
            .into_any_element()
    });
    let painted: Vec<_> = events
        .borrow()
        .iter()
        .filter_map(|&(phase, name)| (phase == "paint").then_some(name))
        .collect();
    assert_eq!(painted, ["before", "flow", "after", "line"]);
    assert!(deferred.complete(), "the line was painted after the block");
    assert_eq!(deferred.line_count(), 1);
}

#[test]
fn consecutive_lines_of_a_block_are_kept_as_one_output_range() {
    let events = Events::default();
    let deferred = captured(&events, || {
        div()
            .child(inline(probe("first", &events)))
            .child(probe("flow", &events))
            .child(inline(probe("second", &events)))
            .into_any_element()
    });
    let painted: Vec<_> = events
        .borrow()
        .iter()
        .filter_map(|&(phase, name)| (phase == "paint").then_some(name))
        .collect();
    assert_eq!(painted, ["before", "flow", "after", "first", "second"]);
    assert!(deferred.complete());
    // Both jobs ran one after the other at the end of the stacking context:
    // their output is one range, not one record per line.
    assert_eq!(deferred.line_count(), 1);
}

#[test]
fn lines_drained_inside_the_block_stay_in_its_output() {
    let events = Events::default();
    let deferred = captured(&events, || {
        collect(vec![inline(probe("line", &events))]).into_any_element()
    });
    assert!(deferred.complete());
    assert_eq!(
        deferred.line_count(),
        0,
        "an inner stacking context drained it"
    );
}

#[test]
fn positioned_paint_deferred_past_the_block_leaves_the_record_incomplete() {
    let events = Events::default();
    let deferred = captured(&events, || {
        div()
            .child(positioned("positioned", &events))
            .into_any_element()
    });
    assert!(!deferred.complete());
}

#[test]
fn lines_that_defer_paint_of_their_own_leave_the_record_incomplete() {
    let events = Events::default();
    let deferred = captured(&events, || {
        let line = div().child(positioned("positioned", &events));
        inline(line.into_any_element())
    });
    assert!(!deferred.complete());
}

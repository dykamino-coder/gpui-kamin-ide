//! Порядок текущих CSS consumers: стабильные ключи, atomic export и границы групп.

use super::{Events, assert_phases, collect, draw, probe};
use crate::hooks::{IntoHooked, Wrap};
use gpui::{AnyElement, App, Bounds, IntoElement, ParentElement, Pixels, Styled, Window, div};

fn inline(name: &'static str, events: &Events) -> AnyElement {
    gpui::PaintInline::new(probe(name, events)).into_any_element()
}

fn positioned(name: &'static str, key: u64, events: &Events) -> AnyElement {
    gpui::PaintLast::new(probe(name, events))
        .key(key)
        .into_any_element()
}

#[test]
fn stable_order_preserves_layout_and_prepaint_once() {
    let events = Events::default();
    draw(|| {
        collect(vec![
            positioned("late", 30, &events),
            inline("inline-first", &events),
            probe("flow-first", &events),
            positioned("tie-first", 10, &events),
            inline("inline-second", &events),
            positioned("tie-second", 10, &events),
            probe("flow-second", &events),
        ])
    });
    assert_phases(
        &events,
        &[
            "late",
            "inline-first",
            "flow-first",
            "tie-first",
            "inline-second",
            "tie-second",
            "flow-second",
        ],
        &[
            "flow-first",
            "flow-second",
            "inline-first",
            "inline-second",
            "tie-first",
            "tie-second",
            "late",
        ],
    );
}

#[test]
fn inline_created_while_draining_precedes_the_next_positioned_job() {
    let events = Events::default();
    draw(|| {
        let first = div()
            .child(inline("new-inline", &events))
            .child(probe("positioned-body", &events));
        collect(vec![
            gpui::PaintLast::new(first.into_any_element())
                .key(1)
                .into_any_element(),
            positioned("next", 2, &events),
        ])
    });
    assert_phases(
        &events,
        &["new-inline", "positioned-body", "next"],
        &["positioned-body", "new-inline", "next"],
    );
}

#[test]
fn atomic_box_drains_inline_but_exports_positioned_descendants() {
    let events = Events::default();
    draw(|| {
        let mut atomic = div()
            .child(positioned("exported", 20, &events))
            .child(inline("atomic-inline", &events))
            .child(probe("atomic-flow", &events));
        atomic.style().paint_atomic = Some(true);
        collect(vec![
            atomic.into_any_element(),
            probe("outer-flow", &events),
            positioned("outer-positioned", 10, &events),
        ])
    });
    assert_phases(
        &events,
        &[
            "exported",
            "atomic-inline",
            "atomic-flow",
            "outer-flow",
            "outer-positioned",
        ],
        &[
            "atomic-flow",
            "atomic-inline",
            "outer-flow",
            "outer-positioned",
            "exported",
        ],
    );
}

#[test]
fn stacking_box_finishes_its_collector_before_outer_flow() {
    let events = Events::default();
    draw(|| {
        let mut stacking = div()
            .child(positioned("inner-positioned", 99, &events))
            .child(inline("inner-inline", &events))
            .child(probe("inner-flow", &events));
        stacking.style().paint_stacking = Some(true);
        collect(vec![
            positioned("outer-positioned", 1, &events),
            stacking.into_any_element(),
            probe("outer-flow", &events),
        ])
    });
    assert_phases(
        &events,
        &[
            "outer-positioned",
            "inner-positioned",
            "inner-inline",
            "inner-flow",
            "outer-flow",
        ],
        &[
            "inner-flow",
            "inner-inline",
            "inner-positioned",
            "outer-flow",
            "outer-positioned",
        ],
    );
}

struct Group {
    child: AnyElement,
    events: Events,
}

impl Wrap for Group {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        self.events.borrow_mut().push(("paint", "group-enter"));
        window.paint_group(
            bounds,
            Default::default(),
            0.,
            0.5,
            0,
            &[],
            None,
            None,
            |window| {
                self.child.paint(window, cx);
            },
        );
        self.events.borrow_mut().push(("paint", "group-exit"));
    }
}

#[test]
fn nested_collector_finishes_inside_its_effect_group() {
    let events = Events::default();
    draw(|| {
        let group = Group {
            child: collect(vec![
                positioned("group-positioned", 99, &events),
                inline("group-inline", &events),
                probe("group-flow", &events),
            ])
            .into_any_element(),
            events: events.clone(),
        }
        .hooked();
        collect(vec![
            positioned("outer", 1, &events),
            group,
            probe("flow", &events),
        ])
    });
    assert_phases(
        &events,
        &[
            "outer",
            "group-positioned",
            "group-inline",
            "group-flow",
            "flow",
        ],
        &[
            "group-enter",
            "group-flow",
            "group-inline",
            "group-positioned",
            "group-exit",
            "flow",
            "outer",
        ],
    );
}

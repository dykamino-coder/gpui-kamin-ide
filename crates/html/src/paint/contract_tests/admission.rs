//! Приём jobs во время replay сохраняет минимальный ключ и FIFO одного уровня.

use super::{Events, assert_phases, collect, draw, probe};
use crate::paint::coordinator::{PaintCollect, PaintLast, roots};
use gpui::{AnyElement, ContextScope, IntoElement, ParentElement, div};
use std::rc::Rc;

fn positioned(child: AnyElement, key: u64) -> AnyElement {
    PaintLast::new(child).key(key).into_any_element()
}

#[test]
fn equal_keys_keep_document_admission_order() {
    let events = Events::default();
    draw(|| {
        collect(vec![
            positioned(probe("first", &events), 7),
            positioned(probe("second", &events), 7),
            positioned(probe("third", &events), 7),
        ])
    });
    assert_phases(
        &events,
        &["first", "second", "third"],
        &["first", "second", "third"],
    );
}

#[test]
fn replay_created_jobs_join_minimum_key_queue_and_equal_key_fifo() {
    let events = Events::default();
    draw(|| {
        let replay = div()
            .child(positioned(probe("new-low", &events), 0))
            .child(positioned(probe("new-tie", &events), 10))
            .child(probe("replay-body", &events));
        collect(vec![
            positioned(replay.into_any_element(), 5),
            positioned(probe("existing-tie", &events), 10),
            positioned(probe("late", &events), 20),
        ])
    });
    assert_phases(
        &events,
        &["new-low", "new-tie", "replay-body", "existing-tie", "late"],
        &["replay-body", "new-low", "existing-tie", "new-tie", "late"],
    );
}

#[test]
fn sibling_roots_share_collector_through_document_scopes() {
    let events = Events::default();
    draw(|| {
        let (open, close) = PaintCollect::pair();
        let children = roots(vec![
            open.into_any_element(),
            div()
                .child(positioned(probe("positioned", &events), 0))
                .child(probe("flow", &events))
                .into_any_element(),
            close.into_any_element(),
        ]);
        div().children(children.into_iter().map(|child| {
            crate::hooks::paint_root(
                ContextScope::new(Rc::new(|_| Box::new(())), child).into_any_element(),
            )
        }))
    });
    assert_phases(&events, &["positioned", "flow"], &["flow", "positioned"]);
}

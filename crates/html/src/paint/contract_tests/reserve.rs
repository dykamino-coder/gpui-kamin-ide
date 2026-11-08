//! Очередь строк кадра документа берёт место под собранные сборкой задания
//! одним выделением (`PaintCollect::pair_reserving`), а не растёт удвоением.

use super::draw;
use crate::hooks::{Hooks, IntoHooked};
use crate::paint::coordinator::{PaintCollect, PaintInline, inline_capacities};
use gpui::{App, Bounds, IntoElement, LayoutId, ParentElement, Pixels, Window, div};
use std::{cell::RefCell, rc::Rc};

type Seen = Rc<RefCell<Vec<Vec<usize>>>>;

/// Строка, которая при краске запоминает место очередей строк открытых кадров.
struct Capacity(Seen);

impl Hooks for Capacity {
    type Layout = ();
    type Prepaint = ();

    async fn request_layout_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        _stack: &mut gpui::PaintStack,
    ) -> (LayoutId, ()) {
        (window.request_layout(Default::default(), [], cx), ())
    }

    async fn prepaint_async(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut Window,
        _: &mut App,
        _stack: &mut gpui::PaintStack,
    ) {
    }

    fn paint(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        _: &mut App,
    ) {
        self.0.borrow_mut().push(inline_capacities(window));
    }
}

/// Места очередей, увиденные каждой из `lines` строк сборщика, собранного на `built`.
fn seen(built: u32, lines: usize) -> Vec<Vec<usize>> {
    let seen = Seen::default();
    draw(|| {
        let (open, close) = PaintCollect::pair_reserving(built);
        let lines = (0..lines)
            .map(|_| PaintInline::new(Capacity(seen.clone()).hooked()).into_any_element());
        div().child(open).child(div().children(lines)).child(close)
    });
    seen.take()
}

#[test]
fn a_document_collector_takes_room_for_its_built_lines_at_once() {
    let seen = seen(5, 5);
    assert_eq!(seen.len(), 5, "every line painted");
    assert!(seen.iter().all(|frames| frames == &[5]), "{seen:?}");
}

#[test]
fn lines_beyond_the_built_count_still_queue() {
    let seen = seen(2, 5);
    assert_eq!(seen.len(), 5, "every line painted");
    assert!(seen.iter().all(|frames| frames[0] >= 5), "{seen:?}");
}

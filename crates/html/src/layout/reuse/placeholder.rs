//! Заместитель пропущенного блока: точный лист раскладки без краски.

use super::Record;
use crate::hooks::Hooks;
use gpui::{App, Bounds, LayoutId, Pixels, Window};
use std::cell::Cell;
use std::rc::Rc;

/// Место заместителя в кадре: его лист и запись, которую он повторяет.
pub(super) struct Slot {
    record: Rc<Record>,
    layout: Cell<Option<LayoutId>>,
}

impl Slot {
    pub(super) fn new(record: Rc<Record>) -> Self {
        Self {
            record,
            layout: Cell::new(None),
        }
    }

    pub(super) fn record(&self) -> &Record {
        &self.record
    }

    /// Лист этого кадра; `None` — лист не запрошен (другой масштаб).
    pub(super) fn layout(&self) -> Option<LayoutId> {
        self.layout.get()
    }
}

pub(super) struct Placeholder {
    slot: Rc<Slot>,
}

impl Placeholder {
    pub(super) fn new(slot: Rc<Slot>) -> Self {
        Self { slot }
    }
}

impl Hooks for Placeholder {
    fn paint_boundary(&self) -> bool {
        false
    }

    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        let snapshot = &self.slot.record.snapshot;
        if snapshot.scale() != window.scale_factor() {
            // Лист другого масштаба неточен: проверка кадра отвергнет его.
            return (window.request_layout(gpui::Style::default(), [], cx), ());
        }
        let id = window.request_snapshot_layout(snapshot);
        self.slot.layout.set(Some(id));
        (id, ())
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, _: &mut (), _: &mut Window, _: &mut App) {}

    fn paint(&mut self, _: Bounds<Pixels>, _: &mut (), _: &mut (), _: &mut Window, _: &mut App) {}
}

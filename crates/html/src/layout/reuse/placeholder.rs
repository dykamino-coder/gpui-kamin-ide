//! Заместитель пропущенного блока: точный лист раскладки, а краска либо
//! пропущена (блок вне экрана), либо повторена из прошлого кадра.

use super::{Record, Replay};
use crate::hooks::Hooks;
use gpui::{App, Bounds, LayoutId, Pixels, Window};
use std::cell::Cell;
use std::rc::Rc;

/// Место заместителя в кадре: его лист и запись, которую он повторяет.
pub(super) struct Slot {
    record: Rc<Record>,
    layout: Cell<Option<LayoutId>>,
    /// Кадр, в который повторяется вывод записи (`FlowReuse::validate`);
    /// `None` — блок вне экрана и не рисуется.
    replay: Cell<Option<u64>>,
}

impl Slot {
    pub(super) fn new(record: Rc<Record>) -> Self {
        Self {
            record,
            layout: Cell::new(None),
            replay: Cell::new(None),
        }
    }

    pub(super) fn record(&self) -> &Record {
        &self.record
    }

    /// Лист этого кадра; `None` — лист не запрошен (другой масштаб).
    pub(super) fn layout(&self) -> Option<LayoutId> {
        self.layout.get()
    }

    pub(super) fn set_replay(&self, epoch: Option<u64>) {
        self.replay.set(epoch);
    }

    pub(super) fn replayed(&self) -> bool {
        self.replay.get().is_some()
    }
}

pub(super) struct Placeholder {
    slot: Rc<Slot>,
}

impl Placeholder {
    pub(super) fn new(slot: Rc<Slot>) -> Self {
        Self { slot }
    }

    /// Обновить повтор записи выводом этого кадра.
    fn update(&self, f: impl FnOnce(&mut Replay)) {
        let mut replay = self.slot.record.replay.borrow_mut();
        if let Some(replay) = replay.as_mut() {
            f(replay);
        }
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

    fn prepaint(&mut self, _: Bounds<Pixels>, _: &mut (), window: &mut Window, _: &mut App) {
        if self.slot.replay.get().is_none() {
            return;
        }
        let prepaint = self
            .slot
            .record
            .replay
            .borrow()
            .as_ref()
            .map(|r| r.prepaint.clone());
        if let Some(prepaint) = prepaint {
            let prepaint = window.replay_retained_prepaint(&prepaint);
            self.update(|r| r.prepaint = prepaint);
        }
    }

    fn paint(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        _: &mut App,
    ) {
        let Some(epoch) = self.slot.replay.get() else {
            return;
        };
        let paint = self
            .slot
            .record
            .replay
            .borrow()
            .as_ref()
            .map(|r| r.paint.clone());
        if let Some(paint) = paint {
            let paint = window.replay_retained_paint(&paint);
            self.update(|r| {
                r.paint = paint;
                r.epoch = epoch;
            });
        }
    }
}

//! Заместитель пропущенного блока: точный лист раскладки, а краска либо
//! пропущена (блок вне экрана), либо повторена из прошлого кадра.

use super::Record;
use crate::hooks::{Hooks, LineCapture};
use gpui::{
    App, Bounds, LayoutId, Pixels, Point, RetainedPaint, RetainedPrepaint, ScaledPixels, Window,
};
use std::cell::Cell;
use std::rc::Rc;

/// Вывод блока в кадре `epoch`, который можно повторить в следующем кадре,
/// если блок стоит на том же месте под той же маской (`FlowReuse::validate`):
/// краска GPUI несёт координаты окна и маску. Так повторяется и видимый блок.
#[derive(Clone)]
pub(crate) struct Replay {
    pub(super) prepaint: RetainedPrepaint,
    pub(super) paint: RetainedPaint,
    /// Краска блока, отложенная за его вывод: строки (`paint::coordinator`).
    pub(super) deferred: Rc<LineCapture>,
    pub(super) origin: Point<ScaledPixels>,
    pub(super) mask: Bounds<Pixels>,
    pub(super) epoch: u64,
}

impl Replay {
    /// Вывод записан в кадре перед `epoch` и весь нарисован в нём.
    pub(super) fn follows(&self, epoch: u64) -> bool {
        self.epoch + 1 == epoch && self.deferred.complete()
    }
}

/// Место заместителя в кадре: его лист и запись, которую он повторяет.
pub(super) struct Slot {
    record: Rc<Record>,
    layout: Cell<Option<LayoutId>>,
    /// Кадр, в который повторяется вывод записи (`FlowReuse::validate`);
    /// `None` — блок вне экрана и не рисуется.
    replay: Cell<Option<u64>>,
    /// Лист берёт ширину от потока (`Record::stretch` при другой ширине):
    /// краска записи не повторяется, а чернила шире записанных.
    stretched: bool,
    /// The block's paint-only values changed since the record
    /// (Store::same_tree): the leaf holds, the recorded paint does not.
    repaint: bool,
}

impl Slot {
    pub(super) fn new(record: Rc<Record>, stretched: bool, repaint: bool) -> Self {
        Self {
            record,
            layout: Cell::new(None),
            replay: Cell::new(None),
            stretched,
            repaint,
        }
    }

    pub(super) fn stretched(&self) -> bool {
        self.stretched
    }

    /// The recorded paint of the block may be replayed in place.
    pub(super) fn paint_holds(&self) -> bool {
        !self.stretched && !self.repaint
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
        let id = if self.slot.stretched {
            window.request_snapshot_layout(&snapshot.stretched())
        } else {
            window.request_snapshot_layout(snapshot)
        };
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
        let replay = self
            .slot
            .record
            .replay
            .borrow()
            .as_ref()
            .map(|r| (r.paint.clone(), r.deferred.clone()));
        if let Some((paint, deferred)) = replay {
            // Строки вне открытого кадра рисуются на месте: в выводе блока.
            let (deferred, paint) = window.record_retained_paint(|window| {
                window.replay_retained_paint(&paint);
                crate::hooks::replay_deferred(window, &deferred)
            });
            self.update(|r| {
                r.paint = paint;
                r.deferred = deferred;
                r.epoch = epoch;
            });
        }
    }
}

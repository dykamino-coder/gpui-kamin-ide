//! Задания строк блока, повторяемого на экране (`layout::reuse::replay`).
//!
//! Строки блока потока рисуются в фазе строк его контекста наложения
//! (CSS 2.1 Appendix E, шаг 7): задание встаёт в очередь кадра контекста и
//! рисуется при его сливе, после фона всех блоков, вне диапазона краски
//! самого блока. Повтор блока поэтому держит, кроме своего диапазона, по
//! диапазону на каждое такое задание: при записи слив снимает команды
//! задания, при повторе блок ставит на то же место очереди задание-повтор,
//! и слив копирует его команды в ту же позицию фазы. Так Blink
//! `PaintController` хранит кэшированные элементы отображения по клиенту и
//! фазе и повторяет подпоследовательность чистого поддерева.

use super::*;
use gpui::{App, RasterPaintCtx, RetainedPaint};
use std::cell::RefCell;

/// Команды одного задания строк в последнем кадре, где оно рисовалось.
#[derive(Default)]
pub(crate) struct JobSlot(RefCell<Option<RetainedPaint>>);

impl JobSlot {
    /// Команды сняты и переносимы в следующий кадр.
    pub(crate) fn ready(&self) -> bool {
        self.0
            .borrow()
            .as_ref()
            .is_some_and(RetainedPaint::replayable)
    }
}

/// Сбор заданий строк, которые блок отдал в кадр снаружи себя.
#[derive(Default)]
pub(crate) struct JobTap {
    /// Глубина кадра, открытого снаружи блока; `None` — проход закрыт.
    depth: Cell<Option<usize>>,
    /// Идёт краска самого блока.
    active: Cell<bool>,
    /// Задание ушло туда, куда повтор его не вернёт: позиционированное или
    /// поставленное уже при сливе (тогда его место в очереди другое).
    escaped: Cell<bool>,
    slots: RefCell<Vec<Rc<JobSlot>>>,
}

impl JobTap {
    /// Диапазоны заданий, если все сняты и повторяемы.
    pub(crate) fn finish(&self) -> Option<Rc<[Rc<JobSlot>]>> {
        let slots = self.slots.take();
        (!self.escaped.get() && slots.iter().all(|slot| slot.ready())).then(|| slots.into())
    }
}

/// Краска `f` блока, чьи задания строк снимаются (`JobTap`).
pub(crate) fn tap_jobs<R>(
    window: &mut Window,
    f: impl FnOnce(&mut Window) -> R,
) -> (R, Rc<JobTap>) {
    let tap = Rc::new(JobTap::default());
    let depth = current(window)
        .filter(|pass| pass.is_open(window))
        .map(|pass| pass.jobs.depth());
    tap.depth.set(depth);
    tap.active.set(true);
    let result = window.with_render_context(Some(tap.clone()), f);
    tap.active.set(false);
    (result, tap)
}

/// Место снятия задания строк, поставленного в кадр `pass` сейчас.
pub(super) fn inline_slot(window: &Window, pass: &CssPaintPass) -> Option<Rc<JobSlot>> {
    let tap = window.render_context::<JobTap>()?;
    // Внутренний кадр (атомарная коробка, контекст наложения внутри блока)
    // сливается в диапазоне краски блока.
    if tap.depth.get() != Some(pass.jobs.depth()) {
        return None;
    }
    if !tap.active.get() {
        tap.escaped.set(true);
        return None;
    }
    let slot = Rc::new(JobSlot::default());
    let mut slots = tap.slots.take();
    slots.push(slot.clone());
    tap.slots.replace(slots);
    Some(slot)
}

/// Позиционированное задание под снятием: повтор его не вернёт.
pub(super) fn positioned_escape(window: &Window) {
    if let Some(tap) = window.render_context::<JobTap>() {
        tap.escaped.set(true);
    }
}

/// Повтор заданий строк блока: на то же место очереди кадра.
pub(crate) fn replay_jobs(window: &mut Window, jobs: &[Rc<JobSlot>]) {
    let Some(pass) = current(window).filter(|pass| pass.is_open(window)) else {
        debug_assert!(jobs.is_empty(), "replayed line jobs need an open CSS frame");
        return;
    };
    for slot in jobs {
        pass.jobs.admit_inline(queue::Inline {
            job: queue::Job::Replay(slot.clone()),
            context: window.raster_paint_ctx(),
            tap: None,
        });
    }
}

/// Краска задания в его контексте: со снятием, повтор или как есть.
pub(super) fn paint_inline(
    job: queue::Job,
    tap: Option<Rc<JobSlot>>,
    context: RasterPaintCtx,
    window: &mut Window,
    cx: &mut App,
) {
    window
        .with_raster_paint_ctx(context, |window| match job {
            queue::Job::Paint(mut child) => match tap {
                Some(slot) => {
                    let ((), paint) = window.record_retained_paint(|w| child.paint(w, cx));
                    slot.0.replace(Some(paint));
                }
                None => child.paint(window, cx),
            },
            queue::Job::Replay(slot) => {
                let old = slot.0.take();
                let new = old.map(|paint| window.replay_retained_paint(&paint));
                slot.0.replace(new);
            }
        })
        .expect("CSS inline job stays in its live GPUI scope");
}

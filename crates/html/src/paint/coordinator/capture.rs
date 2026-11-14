//! Краска блока, отложенная за его вывод. Строки блока рисуются на шаге 7
//! CSS 2.1 Appendix E — в конце кадра стопки, после фонов всех блоков потока,
//! — и вывод краски одного блока их не содержит. Запись блока снимает вывод
//! этих заданий отдельно, а повтор ставит его в ту же очередь строк кадра на
//! место исходных заданий. Позиционированные задания (шаг 8), ушедшие в
//! собиратель контекста, снимаются по одному и возвращаются туда же (`jobs`).

mod jobs;

pub(crate) use jobs::JobCapture;
pub(super) use jobs::{claim as claim_job, lost as job_lost, paint as paint_job};

use super::*;
use gpui::RetainedPaint;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Отложенная краска одного блока в одном кадре.
pub(crate) struct LineCapture {
    /// Внешняя запись: задание, ушедшее и за её вывод, снимает и она.
    parent: Option<Rc<LineCapture>>,
    /// Кадров прохода при начале краски блока: задание кадра ниже уходит.
    base: usize,
    /// Краска блока идёт: задание, ушедшее позже, — краска другого места.
    open: Cell<bool>,
    claimed: Cell<u64>,
    pending: Cell<u64>,
    lost: Cell<bool>,
    lines: RefCell<Vec<RetainedPaint>>,
    /// Позиционированные задания блока в порядке прихода (`jobs`).
    jobs: RefCell<Vec<Rc<JobCapture>>>,
    /// Вложенность: 0 — запись блока, выше — запись отложенного заданием.
    level: u8,
}

impl LineCapture {
    fn new(parent: Option<Rc<LineCapture>>, base: usize, level: u8) -> Self {
        Self {
            parent,
            base,
            open: Cell::new(true),
            claimed: Cell::new(0),
            pending: Cell::new(0),
            lost: Cell::new(false),
            lines: RefCell::default(),
            jobs: RefCell::default(),
            level,
        }
    }

    /// Вся отложенная краска блока нарисована в этом кадре и повторяема.
    pub(crate) fn complete(&self) -> bool {
        !self.lost.get()
            && !self.open.get()
            && self.pending.get() == 0
            && self.jobs.borrow().iter().all(|job| job.complete())
    }

    /// Блок ничего не отложил за свой вывод, и запись полна.
    fn nothing(&self) -> bool {
        self.complete() && self.lines.borrow().is_empty() && self.jobs.borrow().is_empty()
    }

    /// Позиционированное задание блока нарисовано; `replayable` — выводом,
    /// который можно повторить.
    fn job_painted(&self, replayable: bool) {
        self.pending.set(self.pending.get().saturating_sub(1));
        if !replayable {
            self.lost.set(true);
        }
    }

    #[cfg(test)]
    pub(crate) fn line_count(&self) -> usize {
        self.lines.borrow().len()
    }

    #[cfg(test)]
    pub(crate) fn job_count(&self) -> usize {
        self.jobs.borrow().len()
    }

    /// Задание строк блока нарисовано выводом `range`; `own` — только им.
    pub(super) fn painted(&self, range: RetainedPaint, own: bool) {
        self.pending.set(self.pending.get().saturating_sub(1));
        if !own || !range.replayable() {
            self.lost.set(true);
        }
        // Задания строк блока рисуются подряд в конце кадра стопки: их вывод
        // обычно — один непрерывный диапазон, и повтор берёт его целиком.
        let mut lines = self.lines.borrow_mut();
        if lines.last_mut().is_some_and(|last| last.append(&range)) {
            return;
        }
        if lines.capacity() == 0 {
            lines.reserve_exact(1);
        }
        lines.push(range);
    }
}

/// Записи блоков, из вывода которых уходит задание строк, принимаемое сейчас
/// в последний кадр `pass`.
pub(super) fn claim(window: &Window, pass: &CssPaintPass) -> Box<[Rc<LineCapture>]> {
    let target = pass.jobs.depth().saturating_sub(1);
    let open = window.render_context::<LineCapture>();
    let chain = || {
        std::iter::successors(open.clone(), |deferred| deferred.parent.clone())
            .filter(|deferred| target < deferred.base)
    };
    // Сколько записей заберут задание — столько места, без роста и сжатия.
    let mut claims = Vec::with_capacity(chain().filter(|d| d.open.get()).count());
    for deferred in chain() {
        if deferred.open.get() {
            deferred.claimed.set(deferred.claimed.get() + 1);
            deferred.pending.set(deferred.pending.get() + 1);
            claims.push(deferred);
        } else {
            deferred.lost.set(true);
        }
    }
    claims.into_boxed_slice()
}

/// Краска блока `f` и её отложенная часть. Задание, ушедшее за вывод `f`
/// мимо очередей кадра, делает запись неполной.
pub(crate) fn capture<R>(
    window: &mut Window,
    f: impl FnOnce(&mut Window) -> R,
) -> (R, Rc<LineCapture>) {
    let parent = window.render_context::<LineCapture>();
    capture_in(window, parent, 0, f)
}

/// `capture` под внешней записью `parent` на вложенности `level`.
fn capture_in<R>(
    window: &mut Window,
    parent: Option<Rc<LineCapture>>,
    level: u8,
    f: impl FnOnce(&mut Window) -> R,
) -> (R, Rc<LineCapture>) {
    let pass = current(window);
    let base = pass.as_ref().map_or(0, |pass| pass.jobs.depth());
    let before = pass.as_ref().map(|pass| pass.jobs.admitted_below(base));
    let deferred = Rc::new(LineCapture::new(parent, base, level));
    let result = window.with_render_context(Some(deferred.clone()), f);
    deferred.open.set(false);
    if let (Some(pass), Some(before)) = (pass, before)
        && pass.jobs.admitted_below(base) != before + deferred.claimed.get()
    {
        deferred.lost.set(true);
    }
    (result, deferred)
}

/// Повтор отложенной краски `record` полной записи прошлого кадра: её строки
/// и позиционированные задания встают в очереди открытого кадра, вне него
/// рисуются на месте. Ответ — та же краска этого кадра.
pub(crate) fn replay(window: &mut Window, record: &LineCapture) -> Rc<LineCapture> {
    debug_assert!(record.complete(), "incomplete deferred paint record");
    capture(window, |window| jobs::readmit(window, record)).1
}
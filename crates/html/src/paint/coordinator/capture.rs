//! Краска блока, отложенная за его вывод. Строки блока рисуются на шаге 7
//! CSS 2.1 Appendix E — в конце кадра стопки, после фонов всех блоков потока,
//! — и вывод краски одного блока их не содержит. Запись блока снимает вывод
//! этих заданий отдельно, а повтор ставит его в ту же очередь строк кадра на
//! место исходных заданий. Позиционированные задания (шаги 8–9) блок так не
//! повторяет: их порядок задаёт весь кадр.

use super::*;
use gpui::RetainedPaint;
use queue::{Inline, InlinePaint};
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
}

impl LineCapture {
    fn new(window: &Window, base: usize) -> Self {
        Self {
            parent: window.render_context::<LineCapture>(),
            base,
            open: Cell::new(true),
            claimed: Cell::new(0),
            pending: Cell::new(0),
            lost: Cell::new(false),
            lines: RefCell::default(),
        }
    }

    /// Вся отложенная краска блока нарисована в этом кадре и повторяема.
    pub(crate) fn complete(&self) -> bool {
        !self.lost.get() && !self.open.get() && self.pending.get() == 0
    }

    #[cfg(test)]
    pub(crate) fn line_count(&self) -> usize {
        self.lines.borrow().len()
    }

    /// Задание строк блока нарисовано выводом `range`; `own` — только им.
    pub(super) fn painted(&self, range: RetainedPaint, own: bool) {
        self.pending.set(self.pending.get().saturating_sub(1));
        if !own || !range.replayable() {
            self.lost.set(true);
        }
        self.lines.borrow_mut().push(range);
    }
}

/// Записи блоков, из вывода которых уходит задание строк, принимаемое сейчас
/// в последний кадр `pass`.
pub(super) fn claim(window: &Window, pass: &CssPaintPass) -> Vec<Rc<LineCapture>> {
    let target = pass.jobs.depth().saturating_sub(1);
    let mut claims = Vec::new();
    let mut next = window.render_context::<LineCapture>();
    while let Some(deferred) = next {
        if target < deferred.base {
            if deferred.open.get() {
                deferred.claimed.set(deferred.claimed.get() + 1);
                deferred.pending.set(deferred.pending.get() + 1);
                claims.push(deferred.clone());
            } else {
                deferred.lost.set(true);
            }
        }
        next = deferred.parent.clone();
    }
    claims
}

/// Краска блока `f` и её отложенная часть. Задание, ушедшее за вывод `f`
/// мимо очереди строк (позиционированное), делает запись неполной.
pub(crate) fn capture<R>(
    window: &mut Window,
    f: impl FnOnce(&mut Window) -> R,
) -> (R, Rc<LineCapture>) {
    let pass = current(window);
    let base = pass.as_ref().map_or(0, |pass| pass.jobs.depth());
    let before = pass.as_ref().map(|pass| pass.jobs.admitted_below(base));
    let deferred = Rc::new(LineCapture::new(window, base));
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
/// встают в очередь строк открытого кадра, вне него рисуются на месте.
/// Ответ — та же краска этого кадра.
pub(crate) fn replay(window: &mut Window, record: &LineCapture) -> Rc<LineCapture> {
    debug_assert!(record.complete(), "incomplete deferred paint record");
    capture(window, |window| {
        for line in record.lines.borrow().iter() {
            match current(window).filter(|pass| pass.is_open(window)) {
                Some(pass) => {
                    let job = Inline {
                        paint: InlinePaint::Replay(line.clone()),
                        context: window.raster_paint_ctx(),
                        captures: claim(window, &pass),
                    };
                    pass.jobs.admit_inline(job);
                }
                None => {
                    window.replay_retained_paint(line);
                }
            }
        }
    })
    .1
}

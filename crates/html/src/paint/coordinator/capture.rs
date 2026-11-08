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

impl Drop for LineCapture {
    fn drop(&mut self) {
        // Записи переживают scopes окна: последняя ссылка на глубокую цепь
        // родителей может уйти из retained-дерева за один шаг.
        let mut parent = self.parent.take();
        while let Some(record) = parent {
            let Ok(mut record) = Rc::try_unwrap(record) else {
                break;
            };
            parent = record.parent.take();
        }
    }
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

pub(crate) async fn capture_async<R>(
    window: &mut Window,
    f: impl AsyncFnOnce(&mut Window) -> R,
) -> (R, Rc<LineCapture>) {
    let pass = current(window);
    let base = pass.as_ref().map_or(0, |pass| pass.jobs.depth());
    let before = pass.as_ref().map(|pass| pass.jobs.admitted_below(base));
    let deferred = Rc::new(LineCapture::new(window, base));
    let result = window
        .with_render_context_async(Some(deferred.clone()), f)
        .await;
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
                        paint: InlinePaint::Replay(Box::new(line.clone())),
                        context: window.raster_paint_ctx(&pass.raster),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_capture_parents_drop_on_a_small_stack() {
        std::thread::Builder::new()
            .stack_size(512 * 1024)
            .spawn(|| {
                let mut parent = None;
                for _ in 0..20_000 {
                    parent = Some(Rc::new(LineCapture {
                        parent,
                        base: 0,
                        open: Cell::new(false),
                        claimed: Cell::new(0),
                        pending: Cell::new(0),
                        lost: Cell::new(false),
                        lines: RefCell::default(),
                    }));
                }
                let root = parent.as_ref().unwrap().clone();
                drop(parent);
                assert!(root.complete());
                drop(root);
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

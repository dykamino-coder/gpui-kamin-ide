//! Краска блока, отложенная за его вывод. Строки блока рисуются на шаге 7
//! CSS 2.1 Appendix E — в конце кадра стопки, после фонов всех блоков потока,
//! — и вывод краски одного блока их не содержит. Запись блока снимает вывод
//! этих заданий отдельно, а повтор ставит его в ту же очередь строк кадра на
//! место исходных заданий. Позиционированные задания (шаги 8–9) блок так не
//! повторяет: их порядок задаёт весь кадр.

use super::*;
use gpui::{Bounds, Point, RetainedPaint, ScaledPixels};
use queue::{Inline, InlinePaint};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Where the paint of a moved block goes: its offset from the recorded place
/// and the frame's mask, in device pixels (`Window::retained_placeable`).
pub(crate) type Placement = (Point<ScaledPixels>, Bounds<ScaledPixels>);

/// Paint output `paint` of the last frame at `placed`. The caller checked
/// that it can be placed (`LineCapture::placeable`, a whole offset).
pub(crate) fn replay_placed(window: &mut Window, paint: &RetainedPaint, placed: Placement) {
    let done = window.replay_retained_paint_placed(paint, placed.0, placed.1);
    debug_assert!(done.is_some(), "a checked record is placed");
}

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

    /// Every line of the record can be painted at another place under the
    /// frame's mask `mask` (`Window::retained_paint_placeable`).
    pub(crate) fn placeable(&self, window: &Window, mask: Bounds<ScaledPixels>) -> bool {
        let lines = self.lines.borrow();
        lines
            .iter()
            .all(|line| window.retained_paint_placeable(line, mask))
    }

    /// One record of the complete records `parts` of blocks painted in a
    /// row: their lines in order, as one replay puts them back.
    pub(crate) fn join(parts: &[Rc<LineCapture>]) -> Option<Rc<LineCapture>> {
        let mut lines: Vec<RetainedPaint> = Vec::new();
        for part in parts {
            if !part.complete() {
                return None;
            }
            for line in part.lines.borrow().iter() {
                if !lines.last_mut().is_some_and(|last| last.append(line)) {
                    lines.push(line.clone());
                }
            }
        }
        lines.shrink_to_fit();
        Some(Rc::new(LineCapture {
            parent: None,
            base: 0,
            open: Cell::new(false),
            claimed: Cell::new(0),
            pending: Cell::new(0),
            lost: Cell::new(false),
            lines: RefCell::new(lines),
        }))
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

/// Повтор отложенной краски `record` полной записи прошлого кадра: её строки
/// встают в очередь строк открытого кадра, вне него рисуются на месте.
/// Ответ — та же краска этого кадра. `placed` — блок встал на другое место.
pub(crate) fn replay(
    window: &mut Window,
    record: &LineCapture,
    placed: Option<Placement>,
) -> Rc<LineCapture> {
    debug_assert!(record.complete(), "incomplete deferred paint record");
    capture(window, |window| {
        for line in record.lines.borrow().iter() {
            match current(window).filter(|pass| pass.is_open(window)) {
                Some(pass) => {
                    let paint = match placed {
                        Some(placed) => InlinePaint::Placed(Box::new((line.clone(), placed))),
                        None => InlinePaint::Replay(Box::new(line.clone())),
                    };
                    let job = Inline {
                        paint,
                        context: window.raster_paint_ctx(&pass.raster),
                        captures: claim(window, &pass),
                    };
                    pass.jobs.admit_inline(job);
                }
                None => match placed {
                    Some(placed) => replay_placed(window, line, placed),
                    None => {
                        window.replay_retained_paint(line);
                    }
                },
            }
        }
    })
    .1
}

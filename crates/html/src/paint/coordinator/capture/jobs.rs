//! Позиционированные задания блока (CSS 2.1 Appendix E, шаг 8), ушедшие за
//! его вывод в собиратель контекста наложения: обычная вёрстка — блок с
//! `position: relative` без своего контекста и абсолютные потомки в нём.
//! Запись блока снимает вывод каждого такого задания вместе с тем, что оно
//! отложило само (свои строки и вложенные задания), а повтор возвращает
//! задания в собиратель с записанными ключами и в порядке их прихода: место
//! задания среди заданий соседних блоков по-прежнему решает ключ. Blink
//! держит то же в кэшированной подпоследовательности слоя краски
//! (`PaintLayerPainter::Paint`, paint_layer_painter.cc:412-424): слой без
//! перекраски повторяется целиком, а порядок слоёв задаёт обход контекста.
//! Задание, которое не удалось снять целиком, делает запись блока неполной.

use super::*;
use queue::{Positioned, PositionedPaint};

/// Вложенность записей заданий (задание, отложенное заданием, …): глубже
/// запись блока неполна, и блок собирается заново.
const MAX_LEVEL: u8 = 16;

/// Краска одного позиционированного задания в одном кадре.
pub(crate) struct JobCapture {
    key: u64,
    /// Вывод задания; `None`, пока оно не нарисовано.
    paint: RefCell<Option<RetainedPaint>>,
    /// Отложенное заданием за свой вывод; `None` — ничего.
    deferred: RefCell<Option<Rc<LineCapture>>>,
}

impl JobCapture {
    /// Задание нарисовано, и всё отложенное им тоже.
    pub(super) fn complete(&self) -> bool {
        self.paint.borrow().is_some()
            && self
                .deferred
                .borrow()
                .as_ref()
                .is_none_or(|deferred| deferred.complete())
    }

    /// Вывод записи и возврат отложенного ею в очереди кадра.
    fn replay(&self, window: &mut Window) {
        if let Some(paint) = self.paint.borrow().as_ref() {
            window.replay_retained_paint(paint);
        }
        if let Some(deferred) = self.deferred.borrow().as_ref() {
            readmit(window, deferred);
        }
    }
}

/// Записи блоков, из вывода которых уходит позиционированное задание `key`,
/// принимаемое сейчас в последний кадр `pass`, и его общая запись у них.
pub(in crate::paint::coordinator) fn claim(
    window: &Window,
    pass: &CssPaintPass,
    key: u64,
) -> (Box<[Rc<LineCapture>]>, Option<Rc<JobCapture>>) {
    let owners = super::claim(window, pass);
    if owners.is_empty() {
        return (owners, None);
    }
    if owners.iter().any(|owner| owner.level >= MAX_LEVEL) {
        for owner in owners.iter() {
            owner.job_painted(false);
        }
        return (Box::default(), None);
    }
    let record = Rc::new(JobCapture {
        key,
        paint: RefCell::default(),
        deferred: RefCell::default(),
    });
    for owner in owners.iter() {
        owner.add_job(record.clone());
    }
    (owners, Some(record))
}

/// Краска задания в его окружении: вывод и отложенное им снимаются в
/// `record` для записей `owners`.
pub(in crate::paint::coordinator) fn paint(
    paint: PositionedPaint,
    owners: &[Rc<LineCapture>],
    record: Option<Rc<JobCapture>>,
    window: &mut Window,
    cx: &mut gpui::App,
) {
    let draw = |window: &mut Window, cx: &mut gpui::App| match paint {
        PositionedPaint::Element(mut child) => child.paint(window, cx),
        PositionedPaint::Replay(old) => old.replay(window),
    };
    let Some(record) = record else {
        return draw(window, cx);
    };
    let level = owners.iter().map(|owner| owner.level).max().unwrap_or(0) + 1;
    let ((_, range), deferred) = capture_in(window, None, level, |window| {
        window.record_retained_paint(|window| draw(window, cx))
    });
    let replayable = range.replayable();
    record.paint.replace(Some(range));
    if !deferred.nothing() {
        record.deferred.replace(Some(deferred));
    }
    for owner in owners.iter() {
        owner.job_painted(replayable);
    }
}

/// Задание нарисовано не своим выводом (в чужих сегментах): записи неполны.
pub(in crate::paint::coordinator) fn lost(job: &Positioned) {
    for owner in job.captures.iter() {
        owner.job_painted(false);
    }
}

/// Возврат отложенной краски `record` в очереди открытого кадра: строки — в
/// очередь строк, задания — в собиратель с их ключами, в порядке прихода.
/// Вне открытого кадра всё рисуется на месте, как рисовала бы сборка.
pub(super) fn readmit(window: &mut Window, record: &LineCapture) {
    for line in record.lines.borrow().iter() {
        match current(window).filter(|pass| pass.is_open(window)) {
            Some(pass) => {
                let job = queue::Inline {
                    paint: queue::InlinePaint::Replay(Box::new(line.clone())),
                    context: window.raster_paint_ctx(&pass.raster),
                    captures: super::claim(window, &pass),
                };
                pass.jobs.admit_inline(job);
            }
            None => {
                window.replay_retained_paint(line);
            }
        }
    }
    for job in record.jobs.borrow().iter() {
        match current(window).filter(|pass| pass.is_open(window)) {
            Some(pass) => {
                let (captures, record) = claim(window, &pass, job.key);
                pass.jobs.admit(Positioned {
                    key: job.key,
                    paint: PositionedPaint::Replay(job.clone()),
                    context: window.paint_ctx(),
                    captures,
                    record,
                });
            }
            None => job.replay(window),
        }
    }
}

//! Очереди конечных CSS-заданий: FIFO строк и стабильные ключи документа.

use super::capture::{LineCapture, Placement};
use super::*;
use gpui::{AnyElement, App, PaintCtx, PaintScopeId, RasterPaintCtx, RetainedPaint};
use std::collections::{BTreeMap, VecDeque};

pub(super) struct Positioned {
    pub key: u64,
    pub child: AnyElement,
    pub context: PaintCtx,
}

pub(super) struct Inline {
    pub paint: InlinePaint,
    /// Ссылка на окружение, общее с соседями (`gpui::RasterPaintCtx`), и своё размещение:
    /// задание в несколько слов, без копии стеков окна.
    pub context: RasterPaintCtx,
    /// Записи блоков, из вывода которых ушло это задание (`capture`).
    /// Почти всегда пусто: пустой срез не выделяет памяти и на слово уже
    /// вектора, а заданий в кадре стопки тысячи.
    pub captures: Box<[Rc<LineCapture>]>,
}

/// Строки элемента либо их вывод прошлого кадра (повтор записи блока).
/// Вывод — в куче: заданий в кадре стопки тысячи, а повторов среди них мало.
pub(super) enum InlinePaint {
    Element(AnyElement),
    Replay(Box<RetainedPaint>),
    /// The output moved to another place of this frame (`capture::replay`).
    Placed(Box<(RetainedPaint, Placement)>),
}

pub(super) struct Frame {
    pub scope: PaintScopeId,
    pub inline: VecDeque<Inline>,
    // Упорядоченный индекс вместо повторного квадратичного поиска minimum.
    // Новые задания во время replay входят в ту же CSS-фазу; одинаковые
    // ключи сохраняют приход. Каждый AnyElement изымается ровно один раз.
    pub positioned: BTreeMap<u64, VecDeque<Positioned>>,
    /// Заданий, принятых кадром: запись блока под ним видит, ушли ли её
    /// задания за её вывод (`capture`).
    pub admitted: u64,
    /// Строковых заданий, собранных сборкой под этим кадром (`PaintCollect`):
    /// очередь берёт место под них одним выделением при первом задании,
    /// а не растёт удвоением (как Blink резервирует список отображения,
    /// `PaintController`).
    pub inline_capacity: u32,
}

impl Frame {
    pub fn new(scope: PaintScopeId, inline_capacity: u32) -> Self {
        Self {
            scope,
            inline: VecDeque::new(),
            positioned: BTreeMap::new(),
            admitted: 0,
            inline_capacity,
        }
    }

    pub fn push(&mut self, job: Positioned) {
        self.admitted += 1;
        self.positioned.entry(job.key).or_default().push_back(job);
    }

    pub fn push_inline(&mut self, job: Inline) {
        self.admitted += 1;
        // Место берёт первое собранное задание: повтор записи (`capture::replay`)
        // идёт вместо собранных заданий своего блока, и кадр одних повторов
        // остаётся в пределах их числа.
        if self.inline_capacity != 0 && matches!(job.paint, InlinePaint::Element(_)) {
            let reserve = std::mem::take(&mut self.inline_capacity) as usize;
            self.inline
                .reserve_exact(reserve.saturating_sub(self.inline.len()));
        }
        self.inline.push_back(job);
    }

    pub(super) fn next(&mut self) -> Option<Positioned> {
        let mut entry = self.positioned.first_entry()?;
        let next = entry.get_mut().pop_front();
        if entry.get().is_empty() {
            entry.remove();
        }
        next
    }
}

impl CssPaintPass {
    pub(super) fn drain_inline(&self, depth: usize, window: &mut Window, cx: &mut App) {
        loop {
            let next = self.jobs.pop_inline(depth);
            let Some(Inline {
                paint,
                mut context,
                captures,
            }) = next
            else {
                break;
            };
            // Строки сохраняют glyph/background origin, заливка наследуется
            // от текущего места flush, как и в исходном CSS consumer.
            context.set_fill_transform(window.raster_frame().fill_transform);
            let admitted = self.jobs.admitted_below(depth + 1);
            let range = window
                .with_raster_paint_ctx(context, &self.raster, |window| {
                    window
                        .record_retained_paint(|window| match paint {
                            InlinePaint::Element(mut child) => child.paint(window, cx),
                            InlinePaint::Replay(record) => {
                                window.replay_retained_paint(&record);
                            }
                            InlinePaint::Placed(record) => {
                                capture::replay_placed(window, &record.0, record.1);
                            }
                        })
                        .1
                })
                .expect("CSS inline job stays in its live GPUI scope");
            // Задание, отдавшее свои задания, рисуется не только своим выводом.
            let own = self.jobs.admitted_below(depth + 1) == admitted;
            for capture in captures.iter() {
                capture.painted(range.clone(), own);
            }
        }
    }

    pub(super) fn drain(&self, depth: usize, window: &mut Window, cx: &mut App) {
        loop {
            self.drain_inline(depth, window, cx);
            let next = self.jobs.pop_min(depth);
            let Some(Positioned {
                mut child, context, ..
            }) = next
            else {
                break;
            };
            window
                .with_paint_ctx(context, |window| child.paint(window, cx))
                .expect("CSS positioned job stays in its live GPUI scope");
        }
    }

    /// Слив кадра записи `depth`, чьи позиционированные задания рисуются в сегментах
    /// `late` (`recording::with_late_jobs`); задание, чьи сегменты другие, — на своём месте.
    pub(super) fn drain_late(
        &self,
        depth: usize,
        late: &Rc<[gpui::PaintSegmentSlice]>,
        window: &mut Window,
        cx: &mut App,
    ) {
        loop {
            self.drain_inline(depth, window, cx);
            let next = self.jobs.pop_min(depth);
            let Some(Positioned {
                mut child, context, ..
            }) = next
            else {
                break;
            };
            window
                .with_paint_ctx(context, |window| {
                    let moved =
                        window.with_paint_segment_slices(late.clone(), |w| child.paint(w, cx));
                    if moved.is_err() {
                        child.paint(window, cx);
                    }
                })
                .expect("CSS positioned job stays in its live GPUI scope");
        }
    }

    pub(super) fn finish_atomic(&self, depth: usize, window: &mut Window, cx: &mut App) {
        self.drain_inline(depth, window, cx);
        // Atomic открывается только внутри живого внешнего collector.
        self.jobs.export_atomic(depth);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A frame queues thousands of inline jobs (p02: ~4 700): a job refers to the
    /// environment it shares with its siblings instead of copying it (was ~1.5 KB).
    /// Every collector pair built shares one `Cell<Option<FrameGuard>>`: two words.
    #[test]
    fn a_frame_guard_is_two_words() {
        assert_eq!(size_of::<Option<FrameGuard>>(), 2 * size_of::<usize>());
    }

    #[test]
    fn an_inline_job_is_a_few_words() {
        assert!(size_of::<Inline>() <= 256, "{}", size_of::<Inline>());
        assert!(
            size_of::<RasterPaintCtx>() <= 192,
            "{}",
            size_of::<RasterPaintCtx>()
        );
    }
}

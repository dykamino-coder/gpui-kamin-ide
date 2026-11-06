//! Очереди конечных CSS-заданий: FIFO строк и стабильные ключи документа.

use super::*;
use gpui::{AnyElement, App, PaintCtx, PaintScopeId, RasterPaintCtx};
use std::collections::{BTreeMap, VecDeque};

pub(super) struct Positioned {
    pub key: u64,
    pub child: AnyElement,
    pub context: PaintCtx,
}

pub(super) struct Inline {
    pub job: Job,
    pub context: RasterPaintCtx,
    /// Снятие команд задания для повтора блока на экране (`tap`).
    pub tap: Option<Rc<super::tap::JobSlot>>,
}

/// Задание строк: элемент или повтор команд прошлого кадра.
pub(super) enum Job {
    Paint(AnyElement),
    Replay(Rc<super::tap::JobSlot>),
}

pub(super) struct Frame {
    pub scope: PaintScopeId,
    pub inline: VecDeque<Inline>,
    // Упорядоченный индекс вместо повторного квадратичного поиска minimum.
    // Новые задания во время replay входят в ту же CSS-фазу; одинаковые
    // ключи сохраняют приход. Каждый AnyElement изымается ровно один раз.
    pub positioned: BTreeMap<u64, VecDeque<Positioned>>,
}

impl Frame {
    pub fn new(scope: PaintScopeId) -> Self {
        Self {
            scope,
            inline: VecDeque::new(),
            positioned: BTreeMap::new(),
        }
    }

    pub fn push(&mut self, job: Positioned) {
        self.positioned.entry(job.key).or_default().push_back(job);
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
    pub(super) async fn drain_inline_async(
        &self,
        depth: usize,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
        loop {
            let next = self.jobs.pop_inline(depth);
            let Some(Inline {
                job,
                mut context,
                tap,
            }) = next
            else {
                break;
            };
            // Строки сохраняют glyph/background origin, заливка наследуется
            // от текущего места flush, как и в исходном CSS consumer.
            context.set_fill_transform(window.raster_frame().fill_transform);
            super::tap::paint_inline(job, tap, context, window, cx, stack).await;
        }
    }

    pub(super) async fn drain_async(
        &self,
        depth: usize,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
        loop {
            self.drain_inline_async(depth, window, cx, stack).await;
            let next = self.jobs.pop_min(depth);
            let Some(Positioned {
                mut child, context, ..
            }) = next
            else {
                break;
            };
            window
                .with_paint_ctx_async(context, async |window| {
                    child.paint_async(window, cx, stack).await
                })
                .await
                .expect("CSS positioned job stays in its live GPUI scope");
        }
    }

    /// Слив кадра записи `depth`, чьи позиционированные задания рисуются в сегментах
    /// `late` (`recording::with_late_jobs`); задание, чьи сегменты другие, — на своём месте.
    pub(super) async fn drain_late_async(
        &self,
        depth: usize,
        late: &Rc<[gpui::PaintSegmentSlice]>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
        loop {
            self.drain_inline_async(depth, window, cx, stack).await;
            let next = self.jobs.pop_min(depth);
            let Some(Positioned {
                mut child, context, ..
            }) = next
            else {
                break;
            };
            window
                .with_paint_ctx_async(context, async |window| {
                    let moved = window
                        .with_paint_segment_slices_async(late.clone(), async |w| {
                            child.paint_async(w, cx, stack).await
                        })
                        .await;
                    if moved.is_err() {
                        child.paint_async(window, cx, stack).await;
                    }
                })
                .await
                .expect("CSS positioned job stays in its live GPUI scope");
        }
    }

    pub(super) async fn finish_atomic_async(
        &self,
        depth: usize,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
        self.drain_inline_async(depth, window, cx, stack).await;
        self.jobs.export_atomic(depth);
    }
}

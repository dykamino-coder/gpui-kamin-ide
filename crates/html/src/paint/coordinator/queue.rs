//! Очереди конечных CSS-заданий: FIFO строк и стабильные ключи документа.

use super::*;
use gpui::{AnyElement, App, PaintCtx, RasterPaintCtx};
use std::collections::{BTreeMap, VecDeque};

pub(super) struct Positioned {
    pub key: u64,
    pub child: AnyElement,
    pub context: PaintCtx,
}

pub(super) struct Inline {
    pub child: AnyElement,
    pub context: RasterPaintCtx,
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

    fn next(&mut self) -> Option<Positioned> {
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
            let next = self
                .state
                .borrow_mut()
                .frames
                .get_mut(depth)
                .and_then(|f| f.inline.pop_front());
            let Some(Inline {
                mut child,
                mut context,
            }) = next
            else {
                break;
            };
            // Строки сохраняют glyph/background origin, заливка наследуется
            // от текущего места flush, как и в исходном CSS consumer.
            context.set_fill_transform(window.raster_frame().fill_transform);
            window
                .with_raster_paint_ctx(context, |window| child.paint(window, cx))
                .expect("CSS inline job stays in its live GPUI scope");
        }
    }

    pub(super) fn drain(&self, depth: usize, window: &mut Window, cx: &mut App) {
        loop {
            self.drain_inline(depth, window, cx);
            let next = self
                .state
                .borrow_mut()
                .frames
                .get_mut(depth)
                .and_then(Frame::next);
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

    pub(super) fn finish_atomic(&self, depth: usize, window: &mut Window, cx: &mut App) {
        self.drain_inline(depth, window, cx);
        // Atomic открывается только внутри живого внешнего collector.
        let mut state = self.state.borrow_mut();
        let moved = std::mem::take(&mut state.frames[depth].positioned);
        for job in moved.into_values().flatten() {
            state.frames[depth - 1].push(job);
        }
    }
}

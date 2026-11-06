//! Элементы повтора: обёртка записи вокруг собранного блока и заместитель,
//! который повторяет его раскладку, prepaint и краску (`super`).

use super::{PendingReplay, ReplaySlot, plain_environment};
use crate::hooks::{Hooks, Wrap};
use crate::paint::coordinator::{replay_jobs, tap_jobs};
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Window};
use std::rc::Rc;
/// Прозрачная обёртка блока: снимает узлы раскладки, диапазоны prepaint и
/// краски этого кадра.
pub(in crate::layout::reuse) struct ReplayRecorder {
    child: AnyElement,
    pending: Rc<PendingReplay>,
}

impl ReplayRecorder {
    pub(in crate::layout::reuse) fn new(child: AnyElement, pending: Rc<PendingReplay>) -> Self {
        Self { child, pending }
    }
}

impl Wrap for ReplayRecorder {
    fn paint_boundary(&self) -> bool {
        false
    }

    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    async fn request_layout_async(&mut self, window: &mut Window, cx: &mut App, stack: &mut gpui::PaintStack) -> LayoutId {
        let start = window.layout_request_position();
        let id = self.child.request_layout_async(window, cx, stack).await;
        let layout = window.capture_layout(start, id);
        self.pending
            .layout
            .replace(layout.map(|layout| (layout, id)));
        id
    }

    async fn prepaint_async(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App, stack: &mut gpui::PaintStack) {
        let callbacks = window.next_frame_callback_count();
        let plain = plain_environment(window);
        let (_, range) = window.record_retained_prepaint_async(async |window| {
            let _ = self.child.prepaint_async(window, cx, stack).await;
        }).await;
        let quiet =
            range.quiet(window).is_some() && callbacks == window.next_frame_callback_count();
        let Some(id) = self.pending.layout.borrow().as_ref().map(|(_, id)| *id) else {
            return;
        };
        if plain && quiet {
            let origin = window.layout_origin_unrounded(id);
            let size = window.layout_size_unrounded(id);
            self.pending.prepaint.replace(Some((range, origin, size)));
            self.pending.mask.replace(Some(window.content_mask()));
        }
    }

    async fn paint_async(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App, stack: &mut gpui::PaintStack) {
        let sink = self.pending.sink.clone();
        let callbacks = window.next_frame_callback_count();
        let ((_, paint), jobs) = window.with_ink_sink_async(sink, async |window| {
            tap_jobs(window, async |window| {
                window.record_retained_paint_async(async |window| self.child.paint_async(window, cx, stack).await).await
            }).await
        }).await;
        let prepaint = self
            .pending
            .prepaint
            .borrow()
            .as_ref()
            .map(|(p, _, _)| p.clone());
        let replayable = prepaint
            .is_some_and(|prepaint| window.retained_replayable(&prepaint, &paint))
            && callbacks == window.next_frame_callback_count();
        if replayable {
            self.pending.paint.replace(Some(paint));
            self.pending.jobs.replace(Some(jobs));
            self.pending
                .env
                .set(Some((window.scale_factor(), window.frame_serial())));
        }
    }
}

/// Заместитель блока: занятые узлы раскладки, повтор prepaint и краски.
pub(in crate::layout::reuse) struct Replay {
    slot: Rc<ReplaySlot>,
}

impl Replay {
    pub(in crate::layout::reuse) fn new(slot: Rc<ReplaySlot>) -> Self {
        Self { slot }
    }
}

impl Hooks for Replay {
    fn paint_boundary(&self) -> bool {
        false
    }

    type Layout = ();
    type Prepaint = ();

    async fn request_layout_async(&mut self, window: &mut Window, cx: &mut App, _stack: &mut gpui::PaintStack) -> (LayoutId, ()) {
        let layout = self.slot.record.ranges(|r| r.layout.clone());
        match layout.and_then(|layout| window.request_retained_layout(&layout)) {
            Some(id) => {
                self.slot.layout.set(Some(id));
                (id, ())
            }
            // Узлы ушли: проверка кадра отвергнет повтор.
            None => (window.request_layout(gpui::Style::default(), [], cx), ()),
        }
    }

    async fn prepaint_async(&mut self, _: Bounds<Pixels>, _: &mut (), window: &mut Window, _: &mut App, _stack: &mut gpui::PaintStack) {
        self.slot
            .record
            .ranges(|r| r.prepaint = window.replay_retained_prepaint(&r.prepaint));
    }

    fn paint(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        _: &mut App,
    ) {
        self.slot.record.ranges(|r| {
            r.paint = window.replay_retained_paint(&r.paint);
            r.serial = window.frame_serial();
        });
        replay_jobs(window, &self.slot.record.jobs);
    }
}

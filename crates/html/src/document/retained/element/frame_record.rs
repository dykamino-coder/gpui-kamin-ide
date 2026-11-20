//! The frame record of `Retained`: prepaint decides between a plain replay,
//! a scrolled replay (`gpui::Window::plan_scrolled_replay`) and a build, and
//! paint replays or records accordingly.

use super::super::record::Record;
use super::super::window_key::WindowKey;
use super::Retained;
use gpui::{App, Bounds, GlobalElementId, Pixels, RetainedPaint, Window, px};
use std::rc::Rc;

impl Retained {
    pub(super) fn prepaint_record(
        &mut self,
        id: &GlobalElementId,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let scroll_inputs = self
            .builder
            .state
            .root_scroll
            .as_ref()
            .map_or(0, |root| root.scroll_inputs.get());
        let key = WindowKey::capture(bounds, window, cx, scroll_inputs);
        let root = window.retained_root_environment();
        // A document under its root scroller records past the viewport, so a
        // wheel step replays the frame moved (`gpui::Window::plan_scrolled_replay`).
        let margin = if self.builder.state.root_scroll.is_some() {
            px(SCROLL_INTEREST_PX)
        } else {
            px(0.)
        };
        window.with_scroll_interest_margin(margin, |window| {
            window.with_element_state::<Record, _>(id, |record, window| {
                let before = window.next_frame_callback_count();
                // The element state lives one frame: without it the window's
                // last frame did not draw the document, and no block replays from it.
                self.builder.store.draw(record.is_some());
                let record = record.filter(|r| r.reusable(&self.slot, &self.key, &key));
                // A scroll node of the record moved: replay it moved when every
                // node stays inside what was recorded, otherwise build.
                let mut plan = None;
                let record = record.filter(|r| {
                    if !window.retained_scrolled(&r.prepaint) {
                        return true;
                    }
                    // A layered record copies from the document's layer.
                    let layer = self.builder.scene_layer();
                    plan = match (&r.layered, &layer) {
                        (Some(inner), Some(layer)) => window.plan_scrolled_replay(
                            &r.prepaint,
                            &r.paint.in_layer(inner),
                            Some(layer),
                        ),
                        (Some(_), None) => None,
                        (None, _) => window.plan_scrolled_replay(&r.prepaint, &r.paint, None),
                    };
                    plan.is_some()
                });
                if let Some(r) = &record
                    && !window.retained_replayable(&r.prepaint, &r.paint)
                {
                    self.slot.unreplayable.set(true);
                    *self.slot.recorded.borrow_mut() = None;
                }
                if self.tree.is_none()
                    && root
                    && !self.slot.unreplayable.get()
                    && let Some(record) = record.filter(|_| self.builder.repaint_boxes(window, cx))
                {
                    self.replay = true;
                    self.builder.store.forget_replays();
                    self.slot.replays.set(self.slot.replays.get() + 1);
                    let prepaint = match &plan {
                        Some(plan) => {
                            window.replay_retained_prepaint_scrolled(&record.prepaint, plan)
                        }
                        None => window.replay_retained_prepaint(&record.prepaint),
                    };
                    self.scroll_plan = plan;
                    return ((), Record { prepaint, ..record });
                }
                // A placeholder that came near the screen cancels the skip: the
                // frame is built again without placeholders (`layout::reuse`).
                if let Some(flow) = self.flow.as_ref()
                    && self.tree.is_some()
                    && !flow.validate(window)
                {
                    flow.note_rebuild();
                    self.tree = None;
                }
                let prepaint = match self.tree.as_mut() {
                    // Laid out as a root before the frame: in place, as in `build_in`.
                    Some(tree) if self.laid_out.is_some() => {
                        window
                            .record_retained_prepaint(|window| {
                                tree.prepaint_at(bounds.origin, window, cx)
                            })
                            .1
                    }
                    Some(tree) => {
                        window
                            .record_retained_prepaint(|window| tree.prepaint(window, cx))
                            .1
                    }
                    None => self.build_in(bounds, window, cx),
                };
                self.scheduled += window.next_frame_callback_count().saturating_sub(before);
                let record = Record {
                    slot: Rc::downgrade(&self.slot),
                    key: self.key.clone(),
                    window: Some(key),
                    prepaint,
                    paint: RetainedPaint::default(),
                    layered: None,
                    root,
                    settled: false,
                };
                ((), record)
            })
        });
    }

    pub(super) fn paint_record(&mut self, id: &GlobalElementId, window: &mut Window, cx: &mut App) {
        window.with_element_state::<Record, _>(id, |record, window| {
            let mut record = record.unwrap_or_default();
            if self.replay {
                match (self.scroll_plan.take(), record.layered.clone()) {
                    (Some(plan), Some(inner)) => {
                        (record.paint, record.layered) = self.builder.replay_paint_scrolled(
                            window,
                            &record.paint.in_layer(&inner),
                            &plan,
                        );
                    }
                    (Some(plan), None) => {
                        record.paint = window.replay_retained_paint_scrolled(&record.paint, &plan)
                    }
                    (None, _) => record.paint = window.replay_retained_paint(&record.paint),
                }
                return ((), record);
            }
            let Some(tree) = self.tree.as_mut() else {
                return ((), record);
            };
            let before = window.next_frame_callback_count();
            let (paint, layered) = self.builder.record_paint(window, |w| tree.paint(w, cx));
            record.layered = layered;
            if let Some(flow) = &self.flow {
                flow.finish();
            }
            self.scheduled += window.next_frame_callback_count().saturating_sub(before);
            record.settled = record.root
                && paint.replayable()
                && self.scheduled == 0
                && self.builder.state.values.frame_ready();
            record.paint = paint;
            record.settled &= !self.slot.unreplayable.get();
            *self.slot.recorded.borrow_mut() = self.key.clone().filter(|_| record.settled);
            ((), record)
        });
    }
}

/// How far past the root scrollport a document records its frame (each
/// side): Blink's cull rect expansion for composited scrollers
/// (`kCullRectPixelDistanceToExpand`, cull_rect.cc). Step 3 sizes it by memory.
const SCROLL_INTEREST_PX: f32 = 4000.;

//! The frame record of `Retained`: prepaint decides between a plain replay,
//! a scrolled replay (`gpui::Window::plan_scrolled_replay`) and a build, and
//! paint replays or records accordingly.

use super::super::record::{FrameMiss, Inputs, Record, Replay};
use super::super::window_key::WindowKey;
use super::Retained;
use gpui::{App, Bounds, GlobalElementId, Pixels, RetainedPaint, Window, px};

impl Retained {
    pub(super) fn prepaint_record(
        &mut self,
        id: &GlobalElementId,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let key = WindowKey::capture(bounds, window, cx, &self.builder.state);
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
                let mut record = record.unwrap_or_default();
                let epoch = record.begin(&self.builder.store, window);
                // A tree the document step built already counted its reason.
                let replay = self.tree.is_none().then(|| {
                    let store = &self.builder.store;
                    let replay = record.replay(&self.slot, self.key.as_ref(), &key, window, store);
                    let boxes = |replay| match self.builder.repaint_boxes(window, cx) {
                        true => Ok(replay),
                        false => Err(FrameMiss::Boxes.noted()),
                    };
                    replay
                        .and_then(boxes)
                        .inspect_err(|miss| self.slot.missed(*miss))
                        .ok()
                });
                if let Some(Some(replay)) = replay {
                    if let Some(inputs) = &record.inputs {
                        window.replay_layout_inputs(&inputs.layout);
                    }
                    let plan = match replay {
                        Replay::Still => None,
                        Replay::Scrolled(plan) => Some(plan),
                    };
                    self.replay = true;
                    self.builder.store.forget_replays();
                    self.slot.replays.set(self.slot.replays.get() + 1);
                    let prepaint = match &plan {
                        Some(plan) => {
                            window.replay_retained_prepaint_scrolled(&record.prepaint, plan)
                        }
                        None => window.replay_retained_prepaint(&record.prepaint),
                    };
                    // A moved replay remaps scene and scroll-node ranges.
                    self.same_prepaint =
                        plan.is_none() && prepaint.preserves_window_slots(&record.prepaint);
                    record.prepaint = prepaint;
                    self.scroll_plan = plan;
                    return ((), record);
                }
                if let Some(checked) = self.checked.take() {
                    if !checked.holds(epoch, Some(&key), window)
                        || !self.layout_inputs.scroll_unchanged()
                    {
                        self.tree = None;
                        self.flow = None;
                    }
                } else if let Some(flow) = &self.flow
                    && !flow.validate(window)
                {
                    flow.note_rebuild();
                    self.tree = None;
                    self.flow = None;
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
                record.inputs = self.key.clone().map(|frame| {
                    Inputs::new(frame, key, window, std::mem::take(&mut self.layout_inputs))
                });
                record.prepaint = prepaint;
                record.paint = RetainedPaint::default();
                record.layered = None;
                record.layer = None;
                record.root = root;
                ((), record)
            })
        });
    }

    pub(super) fn paint_record(&mut self, id: &GlobalElementId, window: &mut Window, cx: &mut App) {
        window.with_element_state::<Record, _>(id, |record, window| {
            let mut record = record.unwrap_or_default();
            if self.replay {
                if self.scroll_plan.is_some() {
                    // Only a plain replay can carry the original block ranges.
                    record.layer = None;
                }
                match (self.scroll_plan.take(), record.layered.clone()) {
                    (Some(plan), Some(inner)) => {
                        (record.paint, record.layered) = self.builder.replay_paint_scrolled(
                            window,
                            &record.paint.in_layer(&inner),
                            &plan,
                        );
                    }
                    (Some(plan), None) => {
                        record.paint = window.replay_retained_paint_scrolled(&record.paint, &plan);
                        self.builder.store.leave_framed_layer();
                    }
                    (None, _) => {
                        record.replay_paint(window, &self.builder.store, self.same_prepaint)
                    }
                }
                record.painted();
                if let Some(inputs) = &record.inputs {
                    inputs.replayed(window);
                }
                return ((), record);
            }
            let Some(tree) = self.tree.as_mut() else {
                return ((), record);
            };
            let before = window.next_frame_callback_count();
            let (paint, layered) = self.builder.record_paint(window, |w| tree.paint(w, cx));
            record.layered = layered;
            record.layer = self.builder.store.layer_version();
            if let Some(flow) = &self.flow {
                flow.finish();
            }
            self.scheduled += window.next_frame_callback_count().saturating_sub(before);
            let refusal = [
                (!record.root, FrameMiss::NotRoot),
                (!paint.replayable_with_groups(), FrameMiss::Commands),
                (self.scheduled != 0, FrameMiss::Scheduled),
                (
                    !self.builder.state.values.frame_ready(),
                    FrameMiss::NotReady,
                ),
            ];
            let refusal = refusal
                .into_iter()
                .find(|(refused, _)| *refused)
                .map(|(_, miss)| miss);
            record.paint = paint;
            record.painted();
            record.publish(&self.slot, refusal);
            ((), record)
        });
    }
}

/// How far past the root scrollport a document records its frame (each
/// side): Blink's cull rect expansion for composited scrollers
/// (`kCullRectPixelDistanceToExpand`, cull_rect.cc). Step 3 sizes it by memory.
const SCROLL_INTEREST_PX: f32 = 4000.;

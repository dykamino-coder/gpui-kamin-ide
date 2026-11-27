//! Check the live frame ranges before preparing layout and again at the draw site.

use super::{FrameKey, FrameMiss, Record, Slot, WindowKey};
use crate::layout::reuse::Store;
use gpui::{ScrollReplay, Window};
use std::rc::Rc;

pub(in crate::document::retained) enum Replay {
    Still,
    Scrolled(ScrollReplay),
}

impl Record {
    pub(in crate::document::retained) fn replay(
        &self,
        slot: &Slot,
        key: Option<&FrameKey>,
        environment: &WindowKey,
        window: &Window,
        store: &Store,
    ) -> Result<Replay, FrameMiss> {
        let published = slot.inputs(key)?;
        let own = self
            .inputs
            .as_ref()
            .filter(|own| Rc::ptr_eq(own, &published));
        let own = own.ok_or_else(|| FrameMiss::Stale.noted())?;
        if !own.matches(environment, window) {
            return Err(FrameMiss::Environment.noted());
        }
        // The pointer entered or left a box with a hover style: the scene layer paints
        // its new hover (`Builder::repaint_boxes`); a frame without one is built.
        if store.scene_layer().is_none() && !own.same_hovers(environment) {
            return Err(FrameMiss::Environment.noted());
        }
        if !window.layout_scroll_replayable(&own.layout, &self.prepaint) {
            return Err(FrameMiss::LayoutScroll.noted());
        }
        // The frame's record carries its composition groups (gpui::Window::replay_groups).
        if !window.retained_replayable_with_groups(&self.prepaint, &self.paint) {
            if crate::why::enabled()
                && let Some(detail) =
                    window.explain_replayable_with_groups(&self.prepaint, &self.paint)
            {
                crate::why::note(FrameMiss::Paint.code(), || detail);
            }
            slot.recorded.take();
            return Err(FrameMiss::Paint.noted());
        }
        if !window.retained_scrolled(&self.prepaint) {
            return Ok(Replay::Still);
        }
        let layer = match &self.layered {
            Some(_) => Some(
                store
                    .scene_layer()
                    .ok_or_else(|| FrameMiss::Paint.noted())?,
            ),
            None => None,
        };
        let layered = self
            .layered
            .as_ref()
            .map(|inner| self.paint.in_layer(inner));
        let paint = layered.as_ref().unwrap_or(&self.paint);
        let plan = window.scroll_replay_plan(&self.prepaint, paint, layer.as_ref());
        plan.map(Replay::Scrolled).map_err(|refusal| {
            let miss = FrameMiss::Scroll(refusal);
            if crate::why::enabled() {
                // gpui names the line that refused and what it saw.
                let explained =
                    window.explain_scroll_refusal(&self.prepaint, paint, layer.as_ref());
                if let Some((_, site, detail)) = explained {
                    crate::why::note_at(miss.code(), site, || detail);
                }
            }
            miss
        })
    }
}

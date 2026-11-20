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
        let own = own.ok_or(FrameMiss::Stale)?;
        if !own.matches(environment, window) {
            return Err(FrameMiss::Environment);
        }
        if !window.layout_scroll_replayable(&own.layout, &self.prepaint) {
            return Err(FrameMiss::LayoutScroll);
        }
        // The frame's record carries its composition groups (gpui::Window::replay_groups).
        if !window.retained_replayable_with_groups(&self.prepaint, &self.paint) {
            slot.unreplayable.set(true);
            slot.recorded.take();
            return Err(FrameMiss::Paint);
        }
        if !window.retained_scrolled(&self.prepaint) {
            return Ok(Replay::Still);
        }
        let plan = match &self.layered {
            Some(inner) => window.scroll_replay_plan(
                &self.prepaint,
                &self.paint.in_layer(inner),
                Some(&store.scene_layer().ok_or(FrameMiss::Paint)?),
            ),
            None => window.scroll_replay_plan(&self.prepaint, &self.paint, None),
        };
        plan.map(Replay::Scrolled).map_err(FrameMiss::Scroll)
    }
}

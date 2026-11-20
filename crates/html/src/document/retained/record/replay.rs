//! Check the live frame ranges before preparing layout and again at the draw site.

use super::{FrameKey, Record, Slot, WindowKey};
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
    ) -> Option<Replay> {
        let published = slot.inputs(key)?;
        let own = self.inputs.as_ref()?;
        if !Rc::ptr_eq(own, &published)
            || !own.matches(environment, window)
            || !window.layout_scroll_replayable(&own.layout, &self.prepaint)
        {
            return None;
        }
        if !window.retained_replayable(&self.prepaint, &self.paint) {
            slot.unreplayable.set(true);
            slot.recorded.take();
            return None;
        }
        if !window.retained_scrolled(&self.prepaint) {
            return Some(Replay::Still);
        }
        let plan = match &self.layered {
            Some(inner) => window.plan_scrolled_replay(
                &self.prepaint,
                &self.paint.in_layer(inner),
                Some(&store.scene_layer()?),
            ),
            None => window.plan_scrolled_replay(&self.prepaint, &self.paint, None),
        };
        plan.map(Replay::Scrolled)
    }
}

//! Gesture status retains paced updates until inertia and queued events end.

use super::*;

impl DirectManipulationEventHandler {
    pub(super) fn status_changed(
        &self,
        viewport: Option<&IDirectManipulationViewport>,
        current: DIRECTMANIPULATION_STATUS,
        previous: DIRECTMANIPULATION_STATUS,
    ) {
        if current == previous {
            return;
        }

        self.active
            .set(current == DIRECTMANIPULATION_RUNNING || current == DIRECTMANIPULATION_INERTIA);

        // A new gesture interrupted inertia, so end the old sequence.
        if current == DIRECTMANIPULATION_RUNNING && previous == DIRECTMANIPULATION_INERTIA {
            self.end_gesture();
        }

        if current == DIRECTMANIPULATION_READY {
            self.end_gesture();

            // Reset the content transform so the viewport is ready for the next gesture.
            // ZoomToRect triggers a second RUNNING -> READY cycle, so prevent an infinite loop here.
            if self.last_scale.get() != 1.0
                || self.last_x_offset.get() != 0.0
                || self.last_y_offset.get() != 0.0
            {
                if let Some(viewport) = viewport {
                    unsafe {
                        viewport
                            .ZoomToRect(
                                0.0,
                                0.0,
                                DEFAULT_VIEWPORT_SIZE as f32,
                                DEFAULT_VIEWPORT_SIZE as f32,
                                false,
                            )
                            .log_err();
                    }
                }
            }

            self.last_scale.set(1.0);
            self.last_x_offset.set(0.0);
            self.last_y_offset.set(0.0);
        }

        if self.active.get() || !self.pending_events.borrow().is_empty() {
            self.demand.request();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DirectManipulationEventHandler;
    use crate::{FrameDemand, FramePacer};
    use gpui::{Modifiers, PlatformInput, ScrollDelta, ScrollWheelEvent, TouchPhase, point, px};
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
        sync::Arc,
    };
    use windows::Win32::{Foundation::HWND, Graphics::DirectManipulation::*};

    #[test]
    fn inertia_retains_demand_and_ready_without_events_stops_it() {
        let demand = Arc::new(FrameDemand::new(Arc::new(FramePacer::default())));
        let active = Rc::new(Cell::new(false));
        let handler = DirectManipulationEventHandler::new(
            HWND::default(),
            Rc::new(Cell::new(1.0)),
            Rc::new(RefCell::new(Vec::new())),
            demand.clone(),
            active.clone(),
        );
        handler.status_changed(None, DIRECTMANIPULATION_RUNNING, DIRECTMANIPULATION_READY);
        assert!(active.get());
        assert!(demand.take());
        handler.status_changed(None, DIRECTMANIPULATION_INERTIA, DIRECTMANIPULATION_RUNNING);
        assert!(active.get());
        assert!(demand.take());
        handler.status_changed(None, DIRECTMANIPULATION_READY, DIRECTMANIPULATION_INERTIA);
        assert!(!active.get());
        assert!(!demand.take());
    }

    #[test]
    fn ready_retains_queued_events_even_after_active_motion_ends() {
        let demand = Arc::new(FrameDemand::new(Arc::new(FramePacer::default())));
        let events = Rc::new(RefCell::new(vec![PlatformInput::ScrollWheel(
            ScrollWheelEvent {
                position: point(px(0.0), px(0.0)),
                delta: ScrollDelta::Pixels(point(px(0.0), px(0.0))),
                modifiers: Modifiers::default(),
                touch_phase: TouchPhase::Ended,
            },
        )]));
        let active = Rc::new(Cell::new(true));
        let handler = DirectManipulationEventHandler::new(
            HWND::default(),
            Rc::new(Cell::new(1.0)),
            events.clone(),
            demand.clone(),
            active.clone(),
        );
        handler.status_changed(None, DIRECTMANIPULATION_READY, DIRECTMANIPULATION_RUNNING);
        assert!(!active.get());
        assert!(demand.take());
        assert_eq!(events.borrow().len(), 1);
    }
}

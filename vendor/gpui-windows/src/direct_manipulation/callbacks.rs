//! DirectManipulation callbacks retain demand through inertia and gesture end.

use super::*;

impl IDirectManipulationViewportEventHandler_Impl for DirectManipulationEventHandler_Impl {
    fn OnViewportStatusChanged(
        &self,
        viewport: windows_core::Ref<'_, IDirectManipulationViewport>,
        current: DIRECTMANIPULATION_STATUS,
        previous: DIRECTMANIPULATION_STATUS,
    ) -> windows_core::Result<()> {
        self.status_changed(viewport.as_ref(), current, previous);
        Ok(())
    }

    fn OnViewportUpdated(
        &self,
        _viewport: windows_core::Ref<'_, IDirectManipulationViewport>,
    ) -> windows_core::Result<()> {
        Ok(())
    }

    fn OnContentUpdated(
        &self,
        _viewport: windows_core::Ref<'_, IDirectManipulationViewport>,
        content: windows_core::Ref<'_, IDirectManipulationContent>,
    ) -> windows_core::Result<()> {
        let content = content.as_ref().ok_or(E_POINTER)?;

        // Get the 6-element content transform: [scale, 0, 0, scale, tx, ty]
        let mut xform = [0.0f32; 6];
        unsafe {
            content.GetContentTransform(&mut xform)?;
        }

        let scale = xform[0];
        let scale_factor = self.scale_factor.get();
        let x_offset = xform[4] / scale_factor;
        let y_offset = xform[5] / scale_factor;

        if scale == 0.0 {
            return Ok(());
        }

        let last_scale = self.last_scale.get();
        let last_x = self.last_x_offset.get();
        let last_y = self.last_y_offset.get();

        if float_equals(scale, last_scale)
            && float_equals(x_offset, last_x)
            && float_equals(y_offset, last_y)
        {
            return Ok(());
        }

        let position = self.mouse_position();
        let modifiers = current_modifiers();

        // Direct Manipulation reports both translation and scale in every content update.
        // Translation values can shift during a pinch due to the zoom center shifting.
        // We classify each gesture as either scroll or pinch and only emit one type of event.
        // We allow Scroll -> Pinch (a pinch can start with a small pan) but not the reverse.
        if !float_equals(scale, 1.0) {
            if self.gesture_kind.get() != GestureKind::Pinch {
                self.end_gesture();
                self.gesture_kind.set(GestureKind::Pinch);
                self.pending_events
                    .borrow_mut()
                    .push(PlatformInput::Pinch(PinchEvent {
                        position,
                        delta: 0.0,
                        modifiers,
                        phase: TouchPhase::Started,
                    }));
            }
        } else if self.gesture_kind.get() == GestureKind::None {
            self.gesture_kind.set(GestureKind::Scroll);
            self.scroll_phase.set(TouchPhase::Started);
        }

        match self.gesture_kind.get() {
            GestureKind::Scroll => {
                let dx = x_offset - last_x;
                let dy = y_offset - last_y;
                let touch_phase = self.scroll_phase.get();
                self.scroll_phase.set(TouchPhase::Moved);
                self.pending_events
                    .borrow_mut()
                    .push(PlatformInput::ScrollWheel(ScrollWheelEvent {
                        position,
                        delta: ScrollDelta::Pixels(point(px(dx), px(dy))),
                        modifiers,
                        touch_phase,
                    }));
            }
            GestureKind::Pinch => {
                let scale_delta = scale / last_scale;
                self.pending_events
                    .borrow_mut()
                    .push(PlatformInput::Pinch(PinchEvent {
                        position,
                        delta: scale_delta - 1.0,
                        modifiers,
                        phase: TouchPhase::Moved,
                    }));
            }
            GestureKind::None => {}
        }

        self.last_scale.set(scale);
        self.last_x_offset.set(x_offset);
        self.last_y_offset.set(y_offset);
        self.demand.request();

        Ok(())
    }
}

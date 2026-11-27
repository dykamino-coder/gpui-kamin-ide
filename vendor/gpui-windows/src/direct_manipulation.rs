use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

mod callbacks;
mod status;

use anyhow::Result;
use gpui::*;
use gpui_util::ResultExt;
use windows::Win32::{
    Foundation::*,
    Graphics::{DirectManipulation::*, Gdi::*},
    System::Com::*,
    UI::{Input::Pointer::*, WindowsAndMessaging::*},
};

use crate::*;

/// Default viewport size in pixels. The actual content size doesn't matter
/// because we're using the viewport only for gesture recognition, not for
/// visual output.
const DEFAULT_VIEWPORT_SIZE: i32 = 1000;

pub(crate) struct DirectManipulationHandler {
    manager: IDirectManipulationManager,
    update_manager: IDirectManipulationUpdateManager,
    viewport: IDirectManipulationViewport,
    _handler_cookie: u32,
    window: HWND,
    scale_factor: Rc<Cell<f32>>,
    pending_events: Rc<RefCell<Vec<PlatformInput>>>,
    demand: Arc<FrameDemand>,
    active: Rc<Cell<bool>>,
}

impl DirectManipulationHandler {
    pub fn new(window: HWND, scale_factor: f32, demand: Arc<FrameDemand>) -> Result<Self> {
        unsafe {
            let manager: IDirectManipulationManager =
                CoCreateInstance(&DirectManipulationManager, None, CLSCTX_INPROC_SERVER)?;

            let update_manager: IDirectManipulationUpdateManager = manager.GetUpdateManager()?;

            let viewport: IDirectManipulationViewport = manager.CreateViewport(None, window)?;

            let configuration = DIRECTMANIPULATION_CONFIGURATION_INTERACTION
                | DIRECTMANIPULATION_CONFIGURATION_TRANSLATION_X
                | DIRECTMANIPULATION_CONFIGURATION_TRANSLATION_Y
                | DIRECTMANIPULATION_CONFIGURATION_TRANSLATION_INERTIA
                | DIRECTMANIPULATION_CONFIGURATION_RAILS_X
                | DIRECTMANIPULATION_CONFIGURATION_RAILS_Y
                | DIRECTMANIPULATION_CONFIGURATION_SCALING;
            viewport.ActivateConfiguration(configuration)?;

            viewport.SetViewportOptions(
                DIRECTMANIPULATION_VIEWPORT_OPTIONS_MANUALUPDATE
                    | DIRECTMANIPULATION_VIEWPORT_OPTIONS_DISABLEPIXELSNAPPING,
            )?;

            let mut rect = RECT {
                left: 0,
                top: 0,
                right: DEFAULT_VIEWPORT_SIZE,
                bottom: DEFAULT_VIEWPORT_SIZE,
            };
            viewport.SetViewportRect(&mut rect)?;

            manager.Activate(window)?;
            viewport.Enable()?;

            let scale_factor = Rc::new(Cell::new(scale_factor));
            let pending_events = Rc::new(RefCell::new(Vec::new()));
            let active = Rc::new(Cell::new(false));

            let event_handler: IDirectManipulationViewportEventHandler =
                DirectManipulationEventHandler::new(
                    window,
                    Rc::clone(&scale_factor),
                    Rc::clone(&pending_events),
                    demand.clone(),
                    active.clone(),
                )
                .into();

            let handler_cookie = viewport.AddEventHandler(Some(window), &event_handler)?;

            update_manager.Update(None)?;

            Ok(Self {
                manager,
                update_manager,
                viewport,
                _handler_cookie: handler_cookie,
                window,
                scale_factor,
                pending_events,
                demand,
                active,
            })
        }
    }

    pub fn set_scale_factor(&self, scale_factor: f32) {
        self.scale_factor.set(scale_factor);
    }

    pub fn on_pointer_hit_test(&self, wparam: WPARAM) {
        unsafe {
            let pointer_id = wparam.loword() as u32;
            let mut pointer_type = POINTER_INPUT_TYPE::default();
            if GetPointerType(pointer_id, &mut pointer_type).is_ok() && pointer_type == PT_TOUCHPAD
            {
                if self.viewport.SetContact(pointer_id).log_err().is_some() {
                    self.demand.request();
                }
            }
        }
    }

    pub fn update(&self) {
        unsafe {
            self.update_manager.Update(None).log_err();
        }
    }

    pub fn drain_events(&self) -> Vec<PlatformInput> {
        let events = std::mem::take(&mut *self.pending_events.borrow_mut());
        if self.active.get() {
            self.demand.request();
        }
        events
    }
}

impl Drop for DirectManipulationHandler {
    fn drop(&mut self) {
        unsafe {
            self.viewport.Stop().log_err();
            self.viewport.Abandon().log_err();
            self.manager.Deactivate(self.window).log_err();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GestureKind {
    None,
    Scroll,
    Pinch,
}

#[windows_core::implement(IDirectManipulationViewportEventHandler)]
struct DirectManipulationEventHandler {
    window: HWND,
    scale_factor: Rc<Cell<f32>>,
    gesture_kind: Cell<GestureKind>,
    last_scale: Cell<f32>,
    last_x_offset: Cell<f32>,
    last_y_offset: Cell<f32>,
    scroll_phase: Cell<TouchPhase>,
    pending_events: Rc<RefCell<Vec<PlatformInput>>>,
    demand: Arc<FrameDemand>,
    active: Rc<Cell<bool>>,
}

impl DirectManipulationEventHandler {
    fn new(
        window: HWND,
        scale_factor: Rc<Cell<f32>>,
        pending_events: Rc<RefCell<Vec<PlatformInput>>>,
        demand: Arc<FrameDemand>,
        active: Rc<Cell<bool>>,
    ) -> Self {
        Self {
            window,
            scale_factor,
            gesture_kind: Cell::new(GestureKind::None),
            last_scale: Cell::new(1.0),
            last_x_offset: Cell::new(0.0),
            last_y_offset: Cell::new(0.0),
            scroll_phase: Cell::new(TouchPhase::Started),
            pending_events,
            demand,
            active,
        }
    }

    fn end_gesture(&self) {
        if self.gesture_kind.get() == GestureKind::None {
            return;
        }
        let position = self.mouse_position();
        let modifiers = current_modifiers();
        match self.gesture_kind.get() {
            GestureKind::Scroll => {
                self.pending_events
                    .borrow_mut()
                    .push(PlatformInput::ScrollWheel(ScrollWheelEvent {
                        position,
                        delta: ScrollDelta::Pixels(point(px(0.0), px(0.0))),
                        modifiers,
                        touch_phase: TouchPhase::Ended,
                    }));
            }
            GestureKind::Pinch => {
                self.pending_events
                    .borrow_mut()
                    .push(PlatformInput::Pinch(PinchEvent {
                        position,
                        delta: 0.0,
                        modifiers,
                        phase: TouchPhase::Ended,
                    }));
            }
            GestureKind::None => {}
        }
        self.gesture_kind.set(GestureKind::None);
    }

    fn mouse_position(&self) -> Point<Pixels> {
        let scale_factor = self.scale_factor.get();
        unsafe {
            let mut point: POINT = std::mem::zeroed();
            let _ = GetCursorPos(&mut point);
            let _ = ScreenToClient(self.window, &mut point);
            logical_point(point.x as f32, point.y as f32, scale_factor)
        }
    }
}

fn float_equals(f1: f32, f2: f32) -> bool {
    const EPSILON_SCALE: f32 = 0.00001;
    (f1 - f2).abs() < EPSILON_SCALE * f1.abs().max(f2.abs()).max(EPSILON_SCALE)
}

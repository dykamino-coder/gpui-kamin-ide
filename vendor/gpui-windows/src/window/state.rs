//! Native window state construction.

use super::*;

impl WindowsWindowState {
    pub(super) fn new(
        hwnd: HWND,
        directx_devices: &DirectXDevices,
        window_params: &CREATESTRUCTW,
        current_cursor: Option<HCURSOR>,
        cursor_visible: Arc<AtomicBool>,
        display: WindowsDisplay,
        min_size: Option<Size<Pixels>>,
        appearance: WindowAppearance,
        disable_direct_composition: bool,
        offscreen: bool,
        invalidate_devices: Arc<AtomicBool>,
        frame_pacer: Arc<FramePacer>,
        draw_coordinator: Rc<DrawCoordinator>,
    ) -> Result<Self> {
        let scale_factor = {
            let monitor_dpi = unsafe { GetDpiForWindow(hwnd) } as f32;
            display
                .forced_scale
                .unwrap_or(monitor_dpi / USER_DEFAULT_SCREEN_DPI as f32)
        };
        let origin = logical_point(window_params.x as f32, window_params.y as f32, scale_factor);
        let logical_size = {
            let physical_size = size(
                DevicePixels(window_params.cx),
                DevicePixels(window_params.cy),
            );
            physical_size.to_pixels(scale_factor)
        };
        let fullscreen_restore_bounds = Bounds {
            origin,
            size: logical_size,
        };
        let border_offset = WindowBorderOffset::default();
        let restore_from_minimized = None;
        let resize_boost_until = None;
        let renderer = if offscreen {
            DirectXRenderer::new_offscreen(hwnd, directx_devices)
        } else {
            DirectXRenderer::new(hwnd, directx_devices, disable_direct_composition)
        }
        .context("Creating DirectX renderer")?;
        let callbacks = Callbacks::default();
        let input_handler = None;
        let pending_surrogate = None;
        let last_reported_modifiers = None;
        let last_reported_capslock = None;
        let hovered = false;
        let click_state = ClickState::new();
        let nc_button_pressed = None;
        let fullscreen = None;
        let initial_placement = None;

        let frame_demand = Arc::new(FrameDemand::new(frame_pacer));
        let direct_manipulation =
            DirectManipulationHandler::new(hwnd, scale_factor, frame_demand.clone())
                .context("initializing Direct Manipulation")?;

        Ok(Self {
            origin: Cell::new(origin),
            logical_size: Cell::new(logical_size),
            fullscreen_restore_bounds: Cell::new(fullscreen_restore_bounds),
            border_offset,
            appearance: Cell::new(appearance),
            background_appearance: Cell::new(WindowBackgroundAppearance::Opaque),
            scale_factor: Cell::new(scale_factor),
            restore_from_minimized: Cell::new(restore_from_minimized),
            resize_boost_until: Cell::new(resize_boost_until),
            min_size,
            callbacks,
            frame_signal: Arc::new(PlatformFrameSignal::new()),
            input_handler: Cell::new(input_handler),
            ime_enabled: Cell::new(true),
            pending_surrogate: Cell::new(pending_surrogate),
            last_reported_modifiers: Cell::new(last_reported_modifiers),
            last_reported_capslock: Cell::new(last_reported_capslock),
            hovered: Cell::new(hovered),
            last_visibility: Cell::new(None),
            renderer: RefCell::new(renderer),
            force_render_pending: Cell::new(false),
            presentation_pending: Cell::new(false),
            click_state,
            current_cursor: Cell::new(current_cursor),
            cursor_visible,
            nc_button_pressed: Cell::new(nc_button_pressed),
            display: Cell::new(display),
            fullscreen: Cell::new(fullscreen),
            initial_placement: Cell::new(initial_placement),
            hwnd,
            invalidate_devices,
            draw_coordinator,
            frame_demand,
            offscreen,
            direct_manipulation,
            a11y: RefCell::new(None),
        })
    }
}

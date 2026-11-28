//! Demand-paced platform frame delivery. Idle does not call DwmFlush.

use super::*;

impl WindowsPlatform {
    pub(super) fn begin_vsync_thread(&self) {
        let Some(directx_devices) = self.inner.state.directx_devices.borrow().clone() else {
            return;
        };
        let Some(direct_write_text_system) = &self.direct_write_text_system else {
            return;
        };
        let stop = Arc::new(AtomicBool::new(false));
        *self.vsync_stop.borrow_mut() = Some(stop.clone());
        let mut directx_device = directx_devices;
        let platform_window: SafeHwnd = self.handle.into();
        let validation_number = self.inner.validation_number;
        let all_windows = Arc::downgrade(&self.raw_window_handles);
        let text_system = Arc::downgrade(direct_write_text_system);
        let invalidate_devices = self.invalidate_devices.clone();
        let pacer = self.frame_pacer.clone();

        let thread = std::thread::Builder::new()
            .name("VSyncProvider".to_owned())
            .spawn(move || {
                let vsync_provider = VSyncProvider::new();
                while pacer.wait(&stop) {
                    let signal_source = vsync_provider.wait_for_vsync();
                    let signal_at = PlatformFrameSignal::capture(scheduler::Instant::now);
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    if check_device_lost(&directx_device.device)
                        || invalidate_devices.fetch_and(false, Ordering::Acquire)
                    {
                        if let Err(err) = handle_gpu_device_lost(
                            &mut directx_device,
                            platform_window.as_raw(),
                            validation_number,
                            &all_windows,
                            &text_system,
                        ) {
                            panic!("Device lost: {err}");
                        }
                    }
                    let Some(all_windows) = all_windows.upgrade() else {
                        break;
                    };
                    for hwnd in all_windows.read().iter() {
                        if !hwnd.frame_demand.take() {
                            continue;
                        }
                        let visible = hwnd.offscreen
                            || unsafe {
                                IsWindowVisible(hwnd.as_raw()).as_bool()
                                    && !IsIconic(hwnd.as_raw()).as_bool()
                            };
                        if !visible || !hwnd.frame_demand.claim_post() {
                            continue;
                        }
                        if let Some(signal_at) = signal_at {
                            hwnd.frame_signal.record(signal_at, signal_source);
                        }
                        post_frame_request(hwnd.as_raw(), &hwnd.frame_demand);
                    }
                }
            })
            .unwrap();
        let mut threads = self.vsync_threads.borrow_mut();
        threads.retain(|thread| !shutdown::thread_has_exited(thread));
        threads.push(thread);
    }
}

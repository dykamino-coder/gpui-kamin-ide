use super::Scrub;
use windows::Win32::Graphics::Dxgi::{DXGI_ERROR_DEVICE_REMOVED, DXGI_ERROR_WAS_STILL_DRAWING};

#[test]
fn readback_waits_only_for_busy_gpu_and_propagates_device_loss() {
    let mut scrub = Scrub {
        pending: Some((1, 1, 0, 1)),
        ..Default::default()
    };
    assert!(
        !scrub
            .readback_ready(Err(DXGI_ERROR_WAS_STILL_DRAWING.into()))
            .unwrap()
    );
    assert!(scrub.pending.is_some());
    let result = scrub.readback_ready(Err(DXGI_ERROR_DEVICE_REMOVED.into()));
    assert!(result.is_err(), "device loss must reach renderer recovery");
    assert!(
        scrub.pending.is_none(),
        "a failed audit must not remain pending"
    );
}

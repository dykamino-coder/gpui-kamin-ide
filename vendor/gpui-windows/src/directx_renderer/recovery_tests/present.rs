//! KaminIDE patch (BR-31A): a frame the renderer did not hand to the
//! compositor is presented by the next paint. Headless: no HWND is created.

use super::super::present_gate::FrameLatencyGate;
use super::*;
use windows::Win32::System::Threading::CreateSemaphoreW;

fn gate(count: i32) -> FrameLatencyGate {
    FrameLatencyGate::new(unsafe { CreateSemaphoreW(None, count, 1, None) }.unwrap())
}

#[test]
fn a_frame_dropped_at_the_gate_is_presented_by_the_next_paint() {
    let device = DirectXDevices::new().unwrap();
    let mut r = renderer(&device);
    let scene = Scene::default();
    // The compositor still holds the previous frame.
    r.resources.as_mut().unwrap().frame_gate = Some(gate(0));
    r.draw(&scene, WindowBackgroundAppearance::Opaque).unwrap();
    assert!(
        r.needs_presentation(),
        "dropped frame must be presented by the next paint"
    );
    // The queue has room again: the next paint presents and clears the flag.
    r.resources.as_mut().unwrap().frame_gate = Some(gate(1));
    r.draw(&scene, WindowBackgroundAppearance::Opaque).unwrap();
    assert!(!r.needs_presentation());
}

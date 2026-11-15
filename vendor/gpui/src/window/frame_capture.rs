//! Оконная привязка единственного host endpoint к backend readback.

use super::Window;
use crate::frame_capture::FrameCaptureSink;

impl Window {
    /// Установить либо отключить readback этого окна; замена закрывает старый endpoint.
    pub fn set_frame_capture(&mut self, sink: Option<Box<dyn FrameCaptureSink>>) -> anyhow::Result<()> {
        // KaminIDE patch: a reader of the read-back frames waits for every
        // drawn frame, so none is left unpresented (`present_unneeded`).
        self.frame_readback = sink.is_some();
        self.platform_window.set_frame_capture(sink)
    }

    /// KaminIDE patch: with a frame capture, let drawn frames that change
    /// nothing on screen go unpresented, as without one: for a reader that
    /// waits only for changed frames and asks for the others
    /// (`Window::require_presentation`).
    pub fn allow_unpresented_readback(&mut self, allowed: bool) {
        self.unpresented_readback = allowed;
    }
}

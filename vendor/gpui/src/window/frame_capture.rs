//! Оконная привязка единственного host endpoint к backend readback.

use super::Window;
use crate::frame_capture::FrameCaptureSink;

impl Window {
    /// Установить либо отключить readback этого окна; замена закрывает старый endpoint.
    pub fn set_frame_capture(&mut self, sink: Option<Box<dyn FrameCaptureSink>>) -> anyhow::Result<()> {
        self.platform_window.set_frame_capture(sink)
    }
}

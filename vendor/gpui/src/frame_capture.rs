//! KaminIDE patch: оконный получатель завершённого BGRA readback.
//!
//! Backend владеет единственным sink. Подписки, sequence и выбор кадра
//! принадлежат host; production offscreen scene не зависит от этого hook.

use std::time::Duration;

/// Завершённая копия клиентского GPU-кадра; строки BGRA идут сверху вниз.
#[derive(Clone)]
pub struct CapturedFrame {
    /// Ширина в device pixels.
    pub width: u32,
    /// Высота в device pixels.
    pub height: u32,
    /// Четыре байта BGRA на пиксель, без padding строк.
    pub bgra: Vec<u8>,
}

/// Эксклюзивный оконный endpoint, перемещаемый в backend без клонирования.
/// Drop endpoint означает завершение/замену capture для этого окна.
pub trait FrameCaptureSink {
    /// Доставить завершённый readback и его длительность.
    fn completed(&mut self, frame: CapturedFrame, readback: Duration);
}

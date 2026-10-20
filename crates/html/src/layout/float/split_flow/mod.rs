//! A paragraph split at a text offset measured during layout: the first
//! formatted line in its own `::first-line` style, the rest below it
//! (`FirstLine`). The container width is known only at measurement, and the
//! tree cannot be built while layout runs, so the measurement computes only
//! the cut and the height; the subtree is built at prepaint.

use gpui::{AnyElement, Pixels};
use std::rc::Rc;
mod first_line;
pub use first_line::FirstLine;

/// Строит поддерево по месту разреза текста (в байтах).
pub type Split = Rc<dyn Fn(usize, Pixels) -> AnyElement>;

/// Где текст расстаётся с плавающим блоком.
#[derive(Clone, Copy, Default)]
struct Cut {
    /// Смещение разреза в байтах.
    at: usize,
    /// Ширина коробки на замере — по ней собирается дерево.
    width: Pixels,
}

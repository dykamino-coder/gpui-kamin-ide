//! Отрисовка строк: выделение, строка, маркеры.

mod line;
mod marker;
mod selection;

/// Память выделения между кадрами: границы в байтах текста абзаца.
#[derive(Default, Clone, Copy)]
pub(super) struct Selection {
    pub(crate) anchor: usize,
    pub(crate) head: usize,
    pub(crate) dragging: bool,
}

impl Selection {
    pub(crate) fn range(&self) -> (usize, usize) {
        (self.anchor.min(self.head), self.anchor.max(self.head))
    }
}

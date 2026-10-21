//! Буферы опросов clamp и результаты обрезки прошлого кадра, принадлежащие одному документу.

use super::ClampLines;

#[derive(Default)]
pub(crate) struct ClampState {
    pub(super) lines: std::cell::RefCell<std::collections::HashMap<u64, ClampLines>>,
    /// Вычисленные точки среза (высота от верха контейнера) прошлого кадра.
    pub(super) cuts: std::cell::RefCell<std::collections::HashMap<u64, f32>>,
    /// Знак обрыва АВТО-режима, посчитанный на прошлом кадре:
    /// ключ контейнера → (номер абзаца, сколько его строк остаётся).
    /// Абзац в контейнере ровно один — тот, на чьей последней строке
    /// перед точкой среза стоит знак (css-overflow-4 §5.3).
    pub(super) para: std::cell::RefCell<std::collections::HashMap<u64, (u32, usize)>>,
    /// Low edge (from the container's content top) of the content placed
    /// BEFORE the clamp point in box-tree order: css-overflow-4 §5.3 hides
    /// only what follows the clamp point, so lines overflowing an earlier
    /// fixed-height block stay visible below the cut (Blink hides the
    /// following boxes for paint instead of clipping the container).
    pub(super) extent: std::cell::RefCell<std::collections::HashMap<u64, f32>>,
    /// Clamp point BETWEEN blocks (no paragraph carries the ellipsis):
    /// container key → number of the first paragraph after the point.
    pub(super) next: std::cell::RefCell<std::collections::HashMap<u64, u32>>,
    /// Boxes fragmented at a clamp point: node → used border-box height
    /// (the content ends at the point; css-overflow-4 §5.3, Blink sizes
    /// every box of the `LineClampAncestorChain`).
    pub(super) boxes: std::cell::RefCell<std::collections::HashMap<u64, f32>>,
}

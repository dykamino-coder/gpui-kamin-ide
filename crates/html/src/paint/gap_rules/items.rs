//! Fragment geometry collected before painting gap decorations.
use gpui::{Bounds, Pixels};
#[derive(Default)]
pub struct GapBuffer {
    pub items: Vec<Bounds<Pixels>>,
    pub multicol: Option<MulticolGaps>,
}
impl std::ops::Deref for GapBuffer {
    type Target = Vec<Bounds<Pixels>>;
    fn deref(&self) -> &Self::Target {
        &self.items
    }
}
impl std::ops::DerefMut for GapBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.items
    }
}
#[derive(Default)]
pub struct MulticolGaps {
    pub columns: Vec<Bounds<Pixels>>,
    pub rows: Vec<(Bounds<Pixels>, usize)>,
    pub spanners: Vec<Bounds<Pixels>>,
}

//! KaminIDE patch: every laid out box of a subtree, for hosts that check
//! that two layout passes placed a subtree the same.

use super::{LayoutId, TaffyLayoutEngine};
use crate::{Bounds, Pixels};

impl TaffyLayoutEngine {
    /// The bounds of `root` and of every node below it, depth first in child
    /// order, as `layout_bounds` gives them.
    pub(crate) fn subtree_bounds(&mut self, root: LayoutId, scale_factor: f32) -> Vec<Bounds<Pixels>> {
        let mut out = Vec::new();
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            out.push(self.layout_bounds(id, scale_factor));
            let children = self.taffy.children_slice(id.into());
            stack.extend(children.iter().rev().map(|&c| LayoutId::from(c)));
        }
        out
    }
}
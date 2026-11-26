//! KaminIDE patch: which descendants carry a box's scrollable overflow, for DevTools `why`.

use super::{LayoutId, TaffyLayoutEngine};
use taffy::style::Overflow;

impl TaffyLayoutEngine {
    /// KaminIDE patch: the node and its ancestors as `display/position size min max`,
    /// for DevTools `why`: what fixed a box's size.
    pub fn layout_ancestry(&self, id: LayoutId, depth: usize) -> Vec<String> {
        let mut node: Option<taffy::NodeId> = Some(id.into());
        let mut steps = Vec::new();
        while let Some(at) = node.filter(|_| steps.len() < depth) {
            let Ok(style) = self.taffy.style(at) else { break };
            steps.push(format!(
                "{:?}/{:?} size {:?} min {:?} max {:?}",
                style.display, style.position, style.size, style.min_size, style.max_size
            ));
            node = self.taffy.parent(at);
        }
        steps
    }

    /// KaminIDE patch: the chain of descendants that reach farthest along the physical
    /// axis (`vertical` false: right edge) in the last layout of `id`, from `id` down,
    /// each as `display/position overflow at offset size -> reach` (layout points,
    /// from `id`'s border box). Diagnostics only: it walks the laid-out tree.
    pub fn overflow_reach(&self, id: LayoutId, vertical: bool, scale_factor: f32) -> Vec<String> {
        let mut steps = Vec::new();
        let mut node: taffy::NodeId = id.into();
        let mut origin = 0.0_f32;
        // A degenerate tree cannot loop: the chain is as deep as the tree.
        for _ in 0..256 {
            let Ok(children) = self.taffy.children(node) else { break };
            let mut best: Option<(taffy::NodeId, f32, f32)> = None;
            for child in children {
                let (Ok(layout), Ok(style)) = (self.taffy.layout(child), self.taffy.style(child)) else {
                    continue;
                };
                let (at, extent, overflow, clip) = if vertical {
                    (layout.location.y, layout.size.height, layout.border.top + layout.scrollable_overflow_rect.bottom, style.overflow.y)
                } else {
                    (layout.location.x, layout.size.width, layout.border.left + layout.scrollable_overflow_rect.right, style.overflow.x)
                };
                // A box that clips its content offers only its border box (CSS Overflow 3 §2.2).
                let reach = at + if clip == Overflow::Visible { extent.max(overflow) } else { extent };
                if best.is_none_or(|(_, _, far)| reach > far) {
                    best = Some((child, at, reach));
                }
            }
            let Some((child, at, reach)) = best else { break };
            let (Ok(layout), Ok(style)) = (self.taffy.layout(child), self.taffy.style(child)) else { break };
            let to = |v: f32| v / scale_factor;
            steps.push(format!(
                "{:?}/{:?} {:?} at {:.1} size {:.1}x{:.1} -> {:.1}",
                style.display,
                style.position,
                style.overflow,
                to(origin + at),
                to(layout.size.width),
                to(layout.size.height),
                to(origin + reach),
            ));
            origin += at;
            node = child;
        }
        steps
    }
}

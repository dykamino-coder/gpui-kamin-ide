//! Visible physical-X lines from completed native geometry, including right-origin metadata.

use super::*;

impl LayoutMeasurement {
    /// Return visible X baselines relative to the root border box in content order.
    pub fn measure_slice_x(
        &mut self,
        available: Size<AvailableSpace>,
        from: Pixels,
        width: Pixels,
        window: &mut Window,
        cx: &mut App,
    ) -> (Size<Pixels>, Option<Pixels>, Option<Pixels>) {
        let measured = self.measure_physical_baselines(available, window, cx);
        let mut lines = Vec::new();
        collect(&self.tree, self.root, 0.0, self.scale_factor, &mut lines);
        let (first, last) = visible(lines, from, width);
        (measured.size, first, last)
    }
}

fn visible(lines: Vec<Pixels>, from: Pixels, width: Pixels) -> (Option<Pixels>, Option<Pixels>) {
    let mut lines = lines.into_iter().filter(|baseline| baseline.0 >= from.0 && baseline.0 <= from.0 + width.0);
    let first = lines.next();
    (first, lines.last().or(first))
}

#[stacksafe::stacksafe]
fn collect(tree: &TaffyTree<NodeContext>, node: NodeId, x: f32, scale: f32, result: &mut Vec<Pixels>) {
    let Some(output) = tree.computed_layout_output(node).expect(EXPECT_MESSAGE) else { return; };
    if output.baselines_x.first.is_none() && output.baselines_x.last.is_none() { return; }
    let children = tree.children(node).expect(EXPECT_MESSAGE);
    if children.is_empty() {
        if let Some(context) = tree.get_node_context(node)
            && let Some(lines) = context.layout_lines_x.as_ref()
        {
            let layout = tree.unrounded_layout(node);
            let left = (layout.padding.left + layout.border.left) / scale;
            let style = tree.style(node).expect(EXPECT_MESSAGE);
            let scrollbar = if style.overflow.y == taffy::Overflow::Scroll { style.scrollbar_width } else { 0.0 };
            let right = (layout.padding.right + layout.border.right + scrollbar) / scale;
            result.extend(lines.iter().map(|baseline| Pixels(x + if context.layout_lines_x_from_right {
                layout.size.width / scale - right - baseline.0
            } else {
                left + baseline.0
            })));
        } else {
            result.extend([output.baselines_x.first, output.baselines_x.last].into_iter().flatten()
                .map(|baseline| Pixels(x + baseline / scale)));
        }
    } else {
        for child in children {
            let style = tree.style(child).expect(EXPECT_MESSAGE);
            if style.position == taffy::Position::Absolute
                || (tree.style(node).expect(EXPECT_MESSAGE).display == taffy::Display::Block && style.float != taffy::Float::None)
            { continue; }
            collect(tree, child, x + tree.unrounded_layout(child).location.x / scale, scale, result);
        }
    }
}

#[cfg(test)]
#[path = "visible_baselines_x_tests.rs"]
mod tests;

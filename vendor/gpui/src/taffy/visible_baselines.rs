//! Read visible content lines from performed-layout metadata, preserving native cache identity.

use super::*;

impl LayoutMeasurement {
    /// Baselines remain relative to the root border box, before the fragment's origin shift.
    pub fn measure_slice(
        &mut self,
        available: Size<AvailableSpace>,
        from: Pixels,
        height: Pixels,
        window: &mut Window,
        cx: &mut App,
    ) -> (Size<Pixels>, Option<Pixels>, Option<Pixels>) {
        reblessive::Stack::new()
            .enter(|stack| self.measure_slice_async(available, from, height, window, cx, stack))
            .finish()
    }

    /// Измеряет видимые линии, продолжая общий стек родительской раскладки.
    pub async fn measure_slice_async(
        &mut self,
        available: Size<AvailableSpace>,
        from: Pixels,
        height: Pixels,
        window: &mut Window,
        cx: &mut App,
        stack: &mut crate::PaintStack,
    ) -> (Size<Pixels>, Option<Pixels>, Option<Pixels>) {
        let measured = self.measure_async(available, window, cx, stack).await;
        let mut lines = Vec::new();
        collect(&self.tree, self.root, 0.0, self.scale_factor, &mut lines);
        let mut visible = lines
            .into_iter()
            .filter(|baseline: &Pixels| baseline.0 >= from.0 && baseline.0 <= from.0 + height.0);
        let first = visible.next();
        let last = visible.last().or(first);
        (measured.0, first, last)
    }
}

// KaminIDE patch: explicit DFS keeps native stack use independent of depth.
fn collect(
    tree: &TaffyTree<NodeContext>,
    node: NodeId,
    y: f32,
    scale: f32,
    result: &mut Vec<Pixels>,
) {
    let mut pending = vec![(node, y)];
    while let Some((node, y)) = pending.pop() {
        let Some(output) = tree.computed_layout_output(node).expect(EXPECT_MESSAGE) else {
            continue;
        };
        if output.baselines.first.is_none() && output.baselines.last.is_none() {
            continue;
        }
        let children = tree.children(node).expect(EXPECT_MESSAGE);
        if children.is_empty() {
            if let Some(lines) = tree
                .get_node_context(node)
                .and_then(|context| context.layout_lines.as_ref())
            {
                let layout = tree.unrounded_layout(node);
                let inset = (layout.padding.top + layout.border.top) / scale;
                result.extend(lines.iter().map(|baseline| Pixels(y + inset + baseline.0)));
            } else {
                result.extend(
                    [output.baselines.first, output.baselines.last]
                        .into_iter()
                        .flatten()
                        .map(|baseline| Pixels(y + baseline / scale)),
                );
            }
        } else {
            for child in children.into_iter().rev() {
                // Only in-flow children nominate block content baselines. Floats are
                // blockified as ordinary items in flex/grid, so skip them only in blocks.
                let style = tree.style(child).expect(EXPECT_MESSAGE);
                if style.position == taffy::Position::Absolute
                    || (tree.style(node).expect(EXPECT_MESSAGE).display == taffy::Display::Block
                        && style.float != taffy::Float::None)
                {
                    continue;
                }
                pending.push((child, y + tree.unrounded_layout(child).location.y / scale));
            }
        }
    }
}

#[cfg(test)]
#[path = "visible_baselines_tests.rs"]
mod tests;

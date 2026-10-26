//! Первая содержательная baseline завершённой раскладки, без нового измерения.

use super::*;

impl TaffyLayoutEngine {
    pub fn first_content_baseline(
        &self,
        root: LayoutId,
        axis: crate::Axis,
        scale: f32,
    ) -> Option<Pixels> {
        first(&self.taffy, root.into(), axis, scale).map(Pixels)
    }
}

// KaminIDE patch: keep the optional stacker feature optional, as in #430.
#[cfg_attr(feature = "stacker", stacksafe::stacksafe)]
fn first(
    tree: &TaffyTree<NodeContext>,
    node: NodeId,
    axis: crate::Axis,
    scale: f32,
) -> Option<f32> {
    let output = tree.computed_layout_output(node).expect(EXPECT_MESSAGE)?;
    let children = tree.children(node).expect(EXPECT_MESSAGE);
    if children.is_empty() {
        return match axis {
            crate::Axis::Vertical => output.baselines.first.map(|v| v / scale),
            crate::Axis::Horizontal => output.baselines_x.first.map(|v| v / scale),
        };
    }
    let parent = tree.style(node).expect(EXPECT_MESSAGE);
    children.into_iter().find_map(|child| {
        let style = tree.style(child).expect(EXPECT_MESSAGE);
        if style.position == taffy::Position::Absolute
            || (parent.display == taffy::Display::Block && style.float != taffy::Float::None)
        {
            return None;
        }
        let offset = tree.unrounded_layout(child).location;
        let offset = match axis {
            crate::Axis::Vertical => offset.y,
            crate::Axis::Horizontal => offset.x,
        } / scale;
        first(tree, child, axis, scale).map(|baseline| offset + baseline)
    })
}

#[cfg(test)]
mod tests;

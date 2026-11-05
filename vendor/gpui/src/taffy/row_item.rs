//! KaminIDE patch: the first baseline a laid-out row gives its single in-flow item, read from
//! the item's stored layout output instead of a baseline probe sibling.

use super::*;

/// The item of a row as baseline alignment sees it (logical pixels).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowItemBaseline {
    /// Distance from the item's top margin edge to its first baseline, as Taffy's flexbox
    /// computes it for a baseline participant (`calculate_children_base_lines`): the first
    /// baseline of the item's output, clamped to the border box for a scroll container, else
    /// the bottom margin edge (css-align-3 §9.1). `None` when the item does not take part in
    /// first-baseline alignment (`align-self` other than `baseline`, or an auto cross margin).
    pub baseline: Option<f32>,
    /// Height of the item's margin box.
    pub outer_height: f32,
}

impl TaffyLayoutEngine {
    /// The first in-flow item of a laid-out row `row` and its baseline; `None` when the row has
    /// no in-flow item.
    pub fn row_item_baseline(&self, row: LayoutId, scale: f32) -> Option<RowItemBaseline> {
        row_item(&self.taffy, row.into(), scale)
    }
}

fn row_item(tree: &TaffyTree<NodeContext>, row: NodeId, scale: f32) -> Option<RowItemBaseline> {
    let parent = tree.style(row).expect(EXPECT_MESSAGE);
    let item = tree
        .children(row)
        .expect(EXPECT_MESSAGE)
        .into_iter()
        .find(|&child| {
            let style = tree.style(child).expect(EXPECT_MESSAGE);
            style.display != taffy::Display::None && style.position != taffy::Position::Absolute
        })?;
    let style = tree.style(item).expect(EXPECT_MESSAGE);
    let layout = tree.layout(item).expect(EXPECT_MESSAGE);
    let height = layout.size.height;
    let outer_height = (height + layout.margin.top + layout.margin.bottom) / scale;
    let align = style.align_self.unwrap_or(parent.align_items);
    let participates = align.keyword == taffy::AlignItemsKeyword::Baseline
        && !style.margin.top.is_auto()
        && !style.margin.bottom.is_auto();
    let baseline = participates.then(|| {
        let first = tree
            .computed_layout_output(item)
            .expect(EXPECT_MESSAGE)
            .and_then(|output| output.baselines.first);
        let inner = first
            .map(|b| {
                if style.overflow.y.is_scroll_container() {
                    b.min(height).max(0.0)
                } else {
                    b
                }
            })
            .unwrap_or(height + layout.margin.bottom);
        (inner + layout.margin.top) / scale
    });
    Some(RowItemBaseline {
        baseline,
        outer_height,
    })
}

#[cfg(test)]
#[path = "row_item_tests.rs"]
mod tests;

//! First/last vertical baseline groups share their start/end grid column edges.
//! Last groups use the final exported baseline and the opposite physical edge.
use super::track_sizing::baseline_coordinate;
use super::types::GridItem;
use crate::geometry::{AbstractAxis, Line, Size};
use crate::style::AlignItemsKeyword;
use crate::style_helpers::TaffyMaxContent;
use crate::tree::{LayoutPartialTree, LayoutPartialTreeExt, SizingMode};
use crate::util::sys::Vec;
use crate::util::ResolveOrZero;

pub(super) fn resolve_item_baselines_x(
    tree: &mut impl LayoutPartialTree,
    items: &mut [GridItem],
    inner_node_size: Size<Option<f32>>,
) {
    resolve_x_groups(tree, items, inner_node_size, false);
    resolve_x_groups(tree, items, inner_node_size, true);
}
fn resolve_x_groups(
    tree: &mut impl LayoutPartialTree,
    items: &mut [GridItem],
    inner_node_size: Size<Option<f32>>,
    last: bool,
) {
    let wanted = if last {
        AlignItemsKeyword::LastBaseline
    } else {
        AlignItemsKeyword::Baseline
    };
    let group_of = |item: &GridItem| {
        let span = item.placement(AbstractAxis::Inline);
        if last {
            span.end
        } else {
            span.start
        }
    };
    // Upstream #1200: the `GridItem`s stay in document order, only references are sorted.
    let mut items: Vec<&mut GridItem> = items.iter_mut().collect();
    items.sort_by_key(|item| group_of(item));
    let mut remaining_items = &mut items[0..];
    while !remaining_items.is_empty() {
        let current_column = group_of(&remaining_items[0]);
        let next_column_first_item = remaining_items
            .iter()
            .position(|item| group_of(item) != current_column);
        let column_items = if let Some(index) = next_column_first_item {
            let (column_items, tail) = remaining_items.split_at_mut(index);
            remaining_items = tail;
            column_items
        } else {
            let column_items = remaining_items;
            remaining_items = &mut [];
            column_items
        };

        for end_side in [false, true] {
            let in_group = move |item: &GridItem| {
                item.justify_self.keyword == wanted
                    && item.participates_in_baseline_alignment_x()
                    && ((item.baseline_x_flags & 1 != 0) != last) == end_side
            };
            // Одиночный участник — запасное выравнивание (`safe self-start`
            // своей стороны), прокладка ему не нужна.
            if column_items.iter().filter(|item| in_group(item)).count() <= 1 {
                continue;
            }
            for item in column_items.iter_mut() {
                if !in_group(item) {
                    continue;
                }
                let measured = tree.perform_child_layout(
                    item.node,
                    Size::NONE,
                    inner_node_size,
                    Size::MAX_CONTENT,
                    SizingMode::InherentSize,
                    Line::FALSE,
                );
                let width = measured.size.width;
                let own = if item.baseline_x_flags & 4 != 0 {
                    if last {
                        measured.last_or_first_x()
                    } else {
                        measured.baselines_x.first
                    }
                } else {
                    None
                };
                let synthesized = if item.baseline_x_flags & 2 != 0 {
                    width / 2.0
                } else {
                    0.0
                };
                let from_left = baseline_coordinate(
                    own.or(Some(synthesized)),
                    width,
                    item.overflow.x.is_scroll_container(),
                );
                item.baseline_x = Some(if end_side {
                    width - from_left
                        + item
                            .margin
                            .right
                            .resolve_or_zero(inner_node_size.width, |val, basis| {
                                tree.calc(val, basis)
                            })
                } else {
                    from_left
                        + item
                            .margin
                            .left
                            .resolve_or_zero(inner_node_size.width, |val, basis| {
                                tree.calc(val, basis)
                            })
                });
            }
            let max_baseline = column_items
                .iter()
                .filter(|item| in_group(item))
                .map(|item| item.baseline_x.unwrap_or(0.0))
                .fold(f32::MIN, f32::max);
            for item in column_items.iter_mut() {
                if !in_group(item) {
                    continue;
                }
                let shim = max_baseline - item.baseline_x.unwrap_or(0.0);
                if end_side {
                    item.baseline_shim_x_end = shim;
                } else {
                    item.baseline_shim_x = shim;
                }
                // Upstream #1226: shims feed the item's known dimensions, so drop the cached entry.
                item.known_dimensions_cache = None;
            }
        }
    }
}

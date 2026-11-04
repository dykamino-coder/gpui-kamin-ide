//! Округление дерева относительно viewport с явными курсорами оставшихся детей.

use crate::geometry::Point;
#[cfg(feature = "content_size")]
use crate::tree::Layout;
use crate::tree::{NodeId, RoundTree};
use crate::util::sys::{round, Vec};

/// Rounds the calculated layout to exact pixel values
///
/// In order to ensure that no gaps in the layout are introduced we:
///   - Always round based on the cumulative x/y coordinates (relative to the viewport) rather than
///     parent-relative coordinates
///   - Compute width/height by first rounding the top/bottom/left/right and then computing the difference
///     rather than rounding the width/height directly
///
/// See <https://github.com/facebook/yoga/commit/aa5b296ac78f7a22e1aeaf4891243c6bb76488e2> for more context
///
/// In order to prevent innacuracies caused by rounding already-rounded values, we read from `unrounded_layout`
/// and write to `final_layout`.
pub fn round_layout(tree: &mut impl RoundTree, node_id: NodeId) {
    let mut pending = Vec::new();
    let mut current = Some((node_id, Point::ZERO));
    while let Some((node, origin)) = current.take() {
        let origin = round_node(tree, node, origin.x, origin.y);
        let child_count = tree.child_count(node);
        let mut cursor =
            Children { node, origin, next: 0, child_count, total_count: child_count + tree.hoisted_child_count(node) };
        loop {
            if let Some(child) = cursor.next(tree) {
                current = Some((child, cursor.origin));
                // Цепочка единственных детей не требует сохранения курсоров и heap-аллокаций.
                if cursor.next < cursor.total_count {
                    pending.push(cursor);
                }
                break;
            }
            match pending.pop() {
                Some(parent) => cursor = parent,
                None => break,
            }
        }
    }
}

/// Курсор детей с координатами их содержащего блока.
struct Children {
    /// Узел, чьи дети обходятся.
    node: NodeId,
    /// Неокруглённое положение узла относительно viewport.
    origin: Point<f32>,
    /// Следующий индекс среди обычных и вынесенных детей.
    next: usize,
    /// Число обычных детей.
    child_count: usize,
    /// Суммарное число обычных и вынесенных детей.
    total_count: usize,
}

impl Children {
    /// Возвращает следующего ребёнка в порядке исходного рекурсивного обхода.
    fn next(&mut self, tree: &impl RoundTree) -> Option<NodeId> {
        while self.next < self.child_count {
            let child = tree.get_child_id(self.node, self.next);
            self.next += 1;
            if !tree.is_out_of_flow(child) {
                return Some(child);
            }
        }
        if self.next < self.total_count {
            let hoisted = self.next - self.child_count;
            self.next += 1;
            Some(tree.get_hoisted_child_id(self.node, hoisted))
        } else {
            None
        }
    }
}

/// Округляет один узел и возвращает исходные накопленные координаты его детей.
fn round_node(tree: &mut impl RoundTree, node_id: NodeId, cumulative_x: f32, cumulative_y: f32) -> Point<f32> {
    let unrounded_layout = tree.get_unrounded_layout(node_id);
    let mut layout = unrounded_layout;

    let parent_x = cumulative_x;
    let parent_y = cumulative_y;
    let cumulative_x = cumulative_x + unrounded_layout.location.x;
    let cumulative_y = cumulative_y + unrounded_layout.location.y;

    layout.location.x = round(cumulative_x) - round(parent_x);
    layout.location.y = round(cumulative_y) - round(parent_y);
    layout.size.width = round(cumulative_x + unrounded_layout.size.width) - round(cumulative_x);
    layout.size.height = round(cumulative_y + unrounded_layout.size.height) - round(cumulative_y);
    layout.scrollbar_size.width = round(unrounded_layout.scrollbar_size.width);
    layout.scrollbar_size.height = round(unrounded_layout.scrollbar_size.height);
    layout.border.left = round(cumulative_x + unrounded_layout.border.left) - round(cumulative_x);
    layout.border.right = round(cumulative_x + unrounded_layout.size.width)
        - round(cumulative_x + unrounded_layout.size.width - unrounded_layout.border.right);
    layout.border.top = round(cumulative_y + unrounded_layout.border.top) - round(cumulative_y);
    layout.border.bottom = round(cumulative_y + unrounded_layout.size.height)
        - round(cumulative_y + unrounded_layout.size.height - unrounded_layout.border.bottom);
    layout.padding.left = round(cumulative_x + unrounded_layout.padding.left) - round(cumulative_x);
    layout.padding.right = round(cumulative_x + unrounded_layout.size.width)
        - round(cumulative_x + unrounded_layout.size.width - unrounded_layout.padding.right);
    layout.padding.top = round(cumulative_y + unrounded_layout.padding.top) - round(cumulative_y);
    layout.padding.bottom = round(cumulative_y + unrounded_layout.size.height)
        - round(cumulative_y + unrounded_layout.size.height - unrounded_layout.padding.bottom);

    #[cfg(feature = "content_size")]
    round_scrollable_overflow_rect(&mut layout, unrounded_layout.scrollable_overflow_rect, cumulative_x, cumulative_y);

    tree.set_final_layout(node_id, &layout);
    Point { x: cumulative_x, y: cumulative_y }
}

#[cfg(feature = "content_size")]
#[inline(always)]
/// Round the scrollable overflow rect.
/// This is split into a separate function to make it easier to feature flag.
fn round_scrollable_overflow_rect(
    layout: &mut Layout,
    unrounded_rect: crate::geometry::Rect<f32>,
    cumulative_x: f32,
    cumulative_y: f32,
) {
    layout.scrollable_overflow_rect.left = round(cumulative_x + unrounded_rect.left) - round(cumulative_x);
    layout.scrollable_overflow_rect.right = round(cumulative_x + unrounded_rect.right) - round(cumulative_x);
    layout.scrollable_overflow_rect.top = round(cumulative_y + unrounded_rect.top) - round(cumulative_y);
    layout.scrollable_overflow_rect.bottom = round(cumulative_y + unrounded_rect.bottom) - round(cumulative_y);
}

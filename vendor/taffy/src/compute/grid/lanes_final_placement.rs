//! Project completed lane stacks onto physical axes and publish child geometry/overflow.
use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn position(
    tree: &mut impl LayoutGridContainer,
    placed: &mut [Placed],
    rows: bool,
    flow: Size<bool>,
    stack_content: f32,
    stack_start_inset: f32,
    border: Rect<f32>,
    #[cfg(feature = "content_size")] is_scroll_container: bool,
) -> Rect<f32> {
    if if rows { flow.width } else { flow.height } {
        for item in placed.iter_mut() {
            // Stretch may have changed size after packing; relative insets remain physical.
            let outer_size = if rows {
                item.size.width + item.margin.horizontal_axis_sum()
            } else {
                item.size.height + item.margin.vertical_axis_sum()
            };
            item.stack_pos = stack_content - item.stack_pos - outer_size;
        }
    }
    #[cfg_attr(not(feature = "content_size"), allow(unused_mut))]
    let mut item_content_size_contribution = Rect::ZERO;
    for (order, p) in placed.iter().enumerate() {
        let (m_start, _) = if rows {
            (p.margin.left, p.margin.right)
        } else {
            (p.margin.top, p.margin.bottom)
        };
        let stack = stack_start_inset + p.stack_pos + m_start + p.stack_relative;
        let location = if rows {
            Point {
                x: stack,
                y: p.grid_pos,
            }
        } else {
            Point {
                x: p.grid_pos,
                y: stack,
            }
        };
        tree.set_unrounded_layout(
            p.node,
            &Layout {
                order: order as u32,
                location,
                size: p.size,
                #[cfg(feature = "content_size")]
                scrollable_overflow_rect: p.scrollable_overflow_rect,
                scrollbar_size: p.scrollbar_size,
                padding: p.padding,
                border: p.border,
                margin: p.margin,
            },
        );
        #[cfg(feature = "content_size")]
        {
            item_content_size_contribution =
                item_content_size_contribution.union(compute_scrollable_overflow_contribution(
                    Point {
                        x: location.x - border.left,
                        y: location.y - border.top,
                    },
                    p.size,
                    p.scrollable_overflow_rect,
                    p.overflow,
                    p.contain,
                    is_scroll_container,
                ));
        }
    }
    item_content_size_contribution
}

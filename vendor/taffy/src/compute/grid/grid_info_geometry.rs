//! Physical areas exported by detailed grid information support both reversed axes.
use super::{DetailedGridInfo, f32_max, f32_min};
use crate::{CheapCloneStr, Direction, GridPlacement, Line, Point, Rect, Size};

impl<S: CheapCloneStr> DetailedGridInfo<S> {
    /// Resolve the physical grid area for an absolutely positioned box from its grid placement.
    /// The padding box and returned area use coordinates relative to the grid container's border box.
    pub fn resolve_absolute_grid_area(
        &self,
        grid_row: Line<GridPlacement<S>>,
        grid_column: Line<GridPlacement<S>>,
        direction: Direction,
        padding_box: Rect<f32>,
    ) -> Rect<f32> {
        let columns = self.columns.resolve_absolute_grid_axis(
            grid_column,
            padding_box.left,
            padding_box.right,
            direction.is_rtl(),
        );
        let rows = self.rows.resolve_absolute_grid_axis(
            grid_row,
            padding_box.top,
            padding_box.bottom,
            self.axis_reversed.height,
        );
        Rect {
            left: columns.start,
            right: columns.end,
            top: rows.start,
            bottom: rows.end,
        }
    }

    /// Compute the location and size of the grid area occupied by the item at `item_index` (an
    /// index into [`DetailedGridInfo::items`]), relative to the grid container's border box.
    ///
    /// The edges resolve to the edges of the tracks bounding the item's grid area (a start line
    /// resolves to the start of the track that follows it and an end line to the end of the track
    /// that precedes it), so the area excludes any gutter or content-alignment spacing around it.
    ///
    /// Returns `None` if `item_index` is out of bounds.
    pub fn item_grid_area(&self, item_index: usize) -> Option<(Point<f32>, Size<f32>)> {
        let item = self.items.get(item_index)?;
        let start_col = self.columns.positions[item.column_start as usize - 1];
        let end_col = self.columns.positions[item.column_end as usize - 2];
        let left = f32_min(start_col.start, end_col.start);
        let right = f32_max(start_col.end, end_col.end);
        let start_row = self.rows.positions[item.row_start as usize - 1];
        let end_row = self.rows.positions[item.row_end as usize - 2];
        let top = f32_min(start_row.start, end_row.start);
        let bottom = f32_max(start_row.end, end_row.end);
        Some((
            Point { x: left, y: top },
            Size {
                width: right - left,
                height: bottom - top,
            },
        ))
    }
}

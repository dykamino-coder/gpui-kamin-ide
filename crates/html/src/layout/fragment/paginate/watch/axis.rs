//! Перевод физической геометрии копии в её логическую блочную ось.

use crate::layout::fragment::types::StackAxis;
use gpui::{Pixels, Point, Size};

#[derive(Clone, Copy)]
pub(super) struct BlockAxis {
    pub(super) origin: Point<Pixels>,
    pub(super) size: Size<Pixels>,
    pub(super) axis: StackAxis,
}

impl BlockAxis {
    pub(super) fn span(self, at: Point<Pixels>, size: Size<Pixels>) -> (f32, f32) {
        let (start, length) = if self.axis.is_vertical() {
            (f32::from(at.x - self.origin.x), f32::from(size.width))
        } else {
            (f32::from(at.y - self.origin.y), f32::from(size.height))
        };
        self.range(start, start + length)
    }

    fn range(self, start: f32, end: f32) -> (f32, f32) {
        if self.axis == StackAxis::VerticalRl {
            let width = f32::from(self.size.width);
            (width - end, width - start)
        } else {
            (start, end)
        }
    }

    pub(super) fn tracks(
        self,
        at: Point<Pixels>,
        columns: Vec<(f32, f32)>,
        rows: Vec<(f32, f32)>,
        table: bool,
    ) -> Vec<(f32, f32)> {
        let (offset, tracks) = if self.axis.is_vertical() {
            (f32::from(at.x - self.origin.x), columns)
        } else {
            (f32::from(at.y - self.origin.y), rows)
        };
        let mut logical: Vec<_> = tracks
            .into_iter()
            .map(|(a, b)| self.range(offset + a, offset + b))
            .collect();
        // Grid сохраняет логический порядок дорожек даже при обратной оси.
        // Таблица зеркалит номера ячеек, её native columns идут слева направо.
        if table && self.axis == StackAxis::VerticalRl {
            logical.reverse();
        }
        logical
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, px, size};

    #[test]
    fn physical_tracks_follow_logical_block_order() {
        let origin = point(px(10.0), px(20.0));
        let at = point(px(17.0), px(31.0));
        for axis in [
            StackAxis::Horizontal,
            StackAxis::VerticalLr,
            StackAxis::VerticalRl,
        ] {
            let projection = BlockAxis {
                origin,
                size: size(px(100.0), px(200.0)),
                axis,
            };
            let columns = vec![(0.0, 20.0), (25.0, 55.0)];
            let rows = vec![(0.0, 30.0), (35.0, 75.0)];
            let expected = match axis {
                StackAxis::Horizontal => vec![(11.0, 41.0), (46.0, 86.0)],
                StackAxis::VerticalLr => vec![(7.0, 27.0), (32.0, 62.0)],
                StackAxis::VerticalRl => vec![(38.0, 68.0), (73.0, 93.0)],
            };
            assert_eq!(
                projection.tracks(at, columns.clone(), rows.clone(), true),
                expected
            );
            let mut grid_columns = columns;
            if axis == StackAxis::VerticalRl {
                grid_columns.reverse();
            }
            assert_eq!(projection.tracks(at, grid_columns, rows, false), expected);
            assert_eq!(
                projection.span(at, size(px(20.0), px(30.0))),
                match axis {
                    StackAxis::Horizontal => (11.0, 41.0),
                    StackAxis::VerticalLr => (7.0, 27.0),
                    StackAxis::VerticalRl => (73.0, 93.0),
                }
            );
        }
    }
}

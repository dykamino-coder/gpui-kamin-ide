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

    pub(super) fn relative_span(self, at: Point<Pixels>, start: f32, length: f32) -> (f32, f32) {
        let offset = if self.axis.is_vertical() {
            f32::from(at.x - self.origin.x)
        } else {
            f32::from(at.y - self.origin.y)
        };
        self.range(offset + start, offset + start + length)
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

/// Продолжение скрывает consumed-space со стороны block-start своей оси.
pub(super) fn consumed_mask(
    axis: StackAxis,
    mut mask: gpui::ContentMask<Pixels>,
    bounds: gpui::Bounds<Pixels>,
    consumed: f32,
    complete: bool,
) -> gpui::ContentMask<Pixels> {
    use gpui::px;
    match axis {
        StackAxis::Horizontal => {
            let end = mask.bounds.bottom();
            let start = if complete {
                end
            } else {
                mask.bounds.origin.y.max(bounds.origin.y + px(consumed))
            };
            mask.bounds.origin.y = start;
            mask.bounds.size.height = (end - start).max(px(0.0));
        }
        StackAxis::VerticalLr => {
            let end = mask.bounds.right();
            let start = if complete {
                end
            } else {
                mask.bounds.origin.x.max(bounds.origin.x + px(consumed))
            };
            mask.bounds.origin.x = start;
            mask.bounds.size.width = (end - start).max(px(0.0));
        }
        StackAxis::VerticalRl => {
            let start = mask.bounds.origin.x;
            let end = if complete {
                start
            } else {
                mask.bounds.right().min(bounds.right() - px(consumed))
            };
            mask.bounds.size.width = (end - start).max(px(0.0));
        }
    }
    mask
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, px, size};

    #[test]
    fn consumed_space_clips_from_logical_block_start() {
        use gpui::{Bounds, ContentMask};
        let bounds = Bounds::new(point(px(10.0), px(20.0)), size(px(100.0), px(200.0)));
        let mask = ContentMask { bounds };
        for axis in [
            StackAxis::Horizontal,
            StackAxis::VerticalLr,
            StackAxis::VerticalRl,
        ] {
            let clipped = consumed_mask(axis, mask, bounds, 30.0, false).bounds;
            match axis {
                StackAxis::Horizontal => {
                    assert_eq!(clipped.origin.y, px(50.0));
                    assert_eq!(clipped.size.height, px(170.0));
                }
                StackAxis::VerticalLr => {
                    assert_eq!(clipped.origin.x, px(40.0));
                    assert_eq!(clipped.size.width, px(70.0));
                }
                StackAxis::VerticalRl => {
                    assert_eq!(clipped.origin.x, px(10.0));
                    assert_eq!(clipped.size.width, px(70.0));
                }
            }
            let complete = consumed_mask(axis, mask, bounds, 30.0, true).bounds;
            assert_eq!(
                if axis.is_vertical() {
                    complete.size.width
                } else {
                    complete.size.height
                },
                px(0.0)
            );
        }
        let nested = ContentMask {
            bounds: Bounds::new(point(px(40.0), px(50.0)), size(px(40.0), px(100.0))),
        };
        for axis in [
            StackAxis::Horizontal,
            StackAxis::VerticalLr,
            StackAxis::VerticalRl,
        ] {
            assert_eq!(consumed_mask(axis, nested, bounds, 20.0, false), nested);
        }
    }

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

//! Матрица листа — единственный источник представления листа: и для воспроизведения
//! записей (`with_paint_presentation`), и для hit testing (`with_hitbox_transform`).

use super::PageStack;
use gpui::{Bounds, Pixels, TransformationMatrix, point, px, size};

impl PageStack {
    /// Logical-pixel matrix of sheet `i`: the stack scale about `bounds.origin`,
    /// preceded by the sheet's `page-orientation` quarter-turn.
    pub(super) fn sheet_matrix(&self, bounds: Bounds<Pixels>, i: usize) -> TransformationMatrix {
        self.sheet_matrix_turned(bounds, i, self.geom(i).turn)
    }

    /// The unrotated stack scale, shared by every sheet.
    pub(super) fn stack_matrix(&self, bounds: Bounds<Pixels>) -> TransformationMatrix {
        self.sheet_matrix_turned(bounds, 0, 0)
    }

    fn sheet_matrix_turned(
        &self,
        bounds: Bounds<Pixels>,
        i: usize,
        turn: u8,
    ) -> TransformationMatrix {
        let s = self.grid.get().1;
        let (bx, by) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
        let g = self.geom(i);
        let (sx, sy) = self.sheet_origin(i);
        let (ox, oy) = (bx + sx, by + sy);
        let (w, h) = g.size;
        let (rotation_scale, translation) = match turn {
            // rotate-right: p -> O + (H - (y - O.y), x - O.x), then q -> B + s(q - B).
            1 => (
                [[0., -s], [s, 0.]],
                [
                    bx * (1. - s) + s * (ox + h + oy),
                    by * (1. - s) + s * (oy - ox),
                ],
            ),
            // rotate-left: p -> O + (y - O.y, W - (x - O.x)).
            3 => (
                [[0., s], [-s, 0.]],
                [
                    bx * (1. - s) + s * (ox - oy),
                    by * (1. - s) + s * (oy + w + ox),
                ],
            ),
            _ => ([[s, 0.], [0., s]], [bx * (1. - s), by * (1. - s)]),
        };
        TransformationMatrix {
            rotation_scale,
            translation,
        }
    }

    /// Page area of sheet `i` in unscaled stack coordinates.
    pub(super) fn page_area(&self, bounds: Bounds<Pixels>, i: usize) -> Bounds<Pixels> {
        let (sx, sy) = self.sheet_origin(i);
        let g = self.geom(i);
        let (ax, ay) = g.area_origin();
        Bounds::new(
            point(bounds.origin.x + px(sx + ax), bounds.origin.y + px(sy + ay)),
            size(px(g.area.0), px(g.area.1)),
        )
    }

    /// The sheet's page area as presented in window coordinates.
    pub(super) fn presented_area(&self, bounds: Bounds<Pixels>, i: usize) -> Bounds<Pixels> {
        let m = self.sheet_matrix(bounds, i);
        let area = self.page_area(bounds, i);
        let (a, z) = (m.apply(area.origin), m.apply(area.bottom_right()));
        Bounds::new(
            point(a.x.min(z.x), a.y.min(z.y)),
            size((z.x - a.x).abs(), (z.y - a.y).abs()),
        )
    }
}

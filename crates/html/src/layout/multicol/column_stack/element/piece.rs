//! Геометрия куска ребёнка стопки — место в колонке и срез `slice` (css-break-3 §4):
//! общая для копий и для записи главной копии.

use crate::layout::fragment::types::Frag;
use crate::layout::multicol::column_stack::ColumnStack;
use crate::tolerance::COORD_EPS;
use gpui::{Bounds, Pixels, Point, Window, point, px, size};

impl ColumnStack {
    /// Начало куска `f` в колонке (до подъёма копии на `from`).
    pub(super) fn piece_at(&self, f: &Frag, bounds: Bounds<Pixels>, step: f32) -> Point<Pixels> {
        let (c, ry) = self.place(f.col);
        let k = &self.children[f.kid];
        point(
            bounds.origin.x + px(c as f32 * step + k.rel.0 + k.par.dx),
            bounds.origin.y + px(ry + f.y + k.rel.1),
        )
    }

    /// Вылет среза вбок: на ширину окна, но не уже стопки (css-multicol-1 §8.1).
    pub(super) fn spill(bounds: Bounds<Pixels>, window: &Window) -> Pixels {
        window.viewport_size().width.max(bounds.size.width)
    }

    /// Срез куска `f`, стоящего в `at` (`slice`, css-break-3 §4): режутся только
    /// внутренние края фрагментов; чернила над первым и под последним видны (§5.4), под
    /// последним — до низа колонки. `flat` — таблица и вложенная фрагментация задают свои
    /// границы. Второе — продолжается ли ребёнок следующим куском.
    pub(super) fn piece_clip(
        &self,
        f: &Frag,
        at: Point<Pixels>,
        (col_w, spill): (f32, Pixels),
        plan_all: &[Frag],
        flat: bool,
    ) -> (Bounds<Pixels>, bool) {
        let line = if self.wraps() { f.col / self.count } else { 0 };
        let line_h = self.lines_plan.borrow().get(line).map_or(0.0, |l| l.1);
        let continued = plan_all
            .iter()
            .any(|g| g.kid == f.kid && g.copy == f.copy + 1);
        let over = self.children[f.kid].over;
        let (up, down) = if flat {
            (0.0, 0.0)
        } else {
            (
                if f.copy == 0 && f.from <= COORD_EPS {
                    f32::from(spill)
                } else {
                    0.0
                },
                if continued || f.from + f.h < over - COORD_EPS {
                    0.0
                } else {
                    (line_h - f.y - f.h).max(0.0)
                },
            )
        };
        let clip = Bounds {
            origin: point(at.x - spill, at.y - px(up)),
            size: size(px(col_w) + spill + spill, px(f.h + up + down)),
        };
        (clip, continued)
    }
}

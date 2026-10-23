//! Итоговое размещение по вставкам или клетке position-area.

use super::super::{AnchorPlan, Placement};
use crate::document::state::ValueCx;
use gpui::{Bounds, Pixels, Window};

impl AnchorPlan {
    /// Размещение коробки по плану: клетка `position-area`, а без области
    /// (или без якоря/содержащего блока для неё) — обе оси по вставкам.
    pub(in crate::layout::positioned::anchor) fn compute(
        &self,
        own: Bounds<Pixels>,
        window: &Window,
        vcx: ValueCx<'_>,
    ) -> Placement {
        if let Some(p) = self.area_place(own, window, vcx) {
            return p;
        }
        let cb = self.cb_bounds(window);
        let (dx, w, ox) = self.free_axis(true, cb, own, vcx);
        let (dy, h, oy) = self.free_axis(false, cb, own, vcx);
        Placement {
            dx,
            dy,
            overflow: ox || oy,
            imcb: (w, h),
            cell: (0.0, 0.0),
        }
    }

    /// Размещение в клетке `position-area`; `None` — области нет или
    /// якорь/содержащий блок не найдены (тогда — обычные `anchor()`-вставки).
    pub(super) fn area_place(
        &self,
        own: Bounds<Pixels>,
        window: &Window,
        vcx: ValueCx<'_>,
    ) -> Option<Placement> {
        let (tx, ty) = self.area?;
        let a = self.lookup(None)?;
        let cb = self.cb_bounds(window)?;
        let (dx, w, ox, cw) = self.axis_place(true, tx, a, cb, own, vcx);
        let (dy, h, oy, ch) = self.axis_place(false, ty, a, cb, own, vcx);
        Some(Placement {
            dx,
            dy,
            overflow: ox || oy,
            imcb: (w, h),
            cell: (cw, ch),
        })
    }
}

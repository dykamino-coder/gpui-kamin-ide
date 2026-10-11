//! Тактики запасных позиций (position-try): итоговое размещение, переворот осей и сторон.

use super::super::area::Tracks;
use super::super::parse::{FLIP_BLOCK, FLIP_INLINE, FLIP_START};
use super::super::{AnchorPlan, Placement};
use crate::style::computed::Align;
use crate::style::values::value::AnchorSide;
use gpui::{Bounds, Pixels, Window};

impl AnchorPlan {
    /// Размещение коробки по плану: клетка `position-area`, а без области
    /// (или без якоря/содержащего блока для неё) — обе оси по вставкам.
    pub(in crate::layout::positioned::anchor) fn compute(
        &self,
        own: Bounds<Pixels>,
        window: &Window,
    ) -> Placement {
        if let Some(p) = self.area_place(own, window) {
            return p;
        }
        let cb = self.cb_bounds(window);
        let (dx, w, ox) = self.free_axis(true, cb, own);
        let (dy, h, oy) = self.free_axis(false, cb, own);
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
    pub(super) fn area_place(&self, own: Bounds<Pixels>, window: &Window) -> Option<Placement> {
        let (tx, ty) = self.area?;
        let a = self.lookup(None)?;
        let cb = self.cb_bounds(window)?;
        let (dx, w, ox, cw) = self.axis_place(true, tx, a, cb, own);
        let (dy, h, oy, ch) = self.axis_place(false, ty, a, cb, own);
        Some(Placement {
            dx,
            dy,
            overflow: ox || oy,
            imcb: (w, h),
            cell: (cw, ch),
        })
    }

    /// Тактика §position-try-fallbacks на готовом плане: `flip-block`/
    /// `flip-inline` — зеркало по оси письма СОДЕРЖАЩЕГО блока («Logical
    /// directions are resolved against the writing mode of the containing
    /// block»): вставки, поля, стороны `anchor()`, дорожки области,
    /// `start`/`end` выравнивания; `flip-start` — транспонирование осей
    /// (диагональ start-start → end-end; для письма с началом не в
    /// левом-верхнем углу — приближение). Составные тактики — по порядку бит.
    pub(in crate::layout::positioned::anchor) fn apply_tactics(&mut self, t: u8) {
        if t & FLIP_BLOCK != 0 {
            self.flip_axis(!self.cb_vertical);
        }
        if t & FLIP_INLINE != 0 {
            self.flip_axis(self.cb_vertical);
        }
        if t & FLIP_START != 0 {
            self.transpose();
        }
    }

    pub(super) fn flip_axis(&mut self, y: bool) {
        let (k1, k2) = if y { (0usize, 2usize) } else { (3, 1) };
        self.sides.swap(k1, k2);
        self.inset.swap(k1, k2);
        self.margin.swap(k1, k2);
        for k in [k1, k2] {
            if let Some(sp) = self.sides[k].as_mut() {
                sp.f.side = mirror_side(sp.f.side, y);
            }
        }
        if let Some((tx, ty)) = self.area.as_mut() {
            let t = if y { ty } else { tx };
            *t = Tracks {
                lo: 3 - t.hi,
                hi: 3 - t.lo,
            };
        }
        // Та же раздача осей, что в `align_for`: `align-self` — блочная ось
        // содержащего блока.
        let slot = if y != self.cb_vertical {
            &mut self.align_self
        } else {
            &mut self.justify_self
        };
        *slot = match *slot {
            Some(Align::Start) => Some(Align::End),
            Some(Align::End) => Some(Align::Start),
            other => other,
        };
    }

    pub(super) fn transpose(&mut self) {
        self.sides.swap(0, 3);
        self.sides.swap(2, 1);
        self.inset.swap(0, 3);
        self.inset.swap(2, 1);
        self.margin.swap(0, 3);
        self.margin.swap(2, 1);
        for sp in self.sides.iter_mut().flatten() {
            sp.f.side = transpose_side(sp.f.side);
        }
        if let Some((tx, ty)) = self.area.as_mut() {
            std::mem::swap(tx, ty);
        }
        std::mem::swap(&mut self.align_self, &mut self.justify_self);
        std::mem::swap(&mut self.safe_align, &mut self.safe_justify);
    }
}

/// Зеркало `<anchor-side>` по оси тактики (§fallback, execute a try-tactic):
/// физические стороны оси, `start`↔`end`, `self-start`↔`self-end`,
/// `<pct>` → `100% − <pct>`; `inside`/`outside` относительны и не меняются.
pub(super) fn mirror_side(s: AnchorSide, y: bool) -> AnchorSide {
    match s {
        AnchorSide::Top if y => AnchorSide::Bottom,
        AnchorSide::Bottom if y => AnchorSide::Top,
        AnchorSide::Left if !y => AnchorSide::Right,
        AnchorSide::Right if !y => AnchorSide::Left,
        AnchorSide::Start => AnchorSide::End,
        AnchorSide::End => AnchorSide::Start,
        AnchorSide::SelfStart => AnchorSide::SelfEnd,
        AnchorSide::SelfEnd => AnchorSide::SelfStart,
        AnchorSide::Pct(p) => AnchorSide::Pct(1.0 - p),
        other => other,
    }
}

/// `flip-start`: физические стороны меняются осями, логические остаются.
pub(super) fn transpose_side(s: AnchorSide) -> AnchorSide {
    match s {
        AnchorSide::Top => AnchorSide::Left,
        AnchorSide::Left => AnchorSide::Top,
        AnchorSide::Bottom => AnchorSide::Right,
        AnchorSide::Right => AnchorSide::Bottom,
        other => other,
    }
}

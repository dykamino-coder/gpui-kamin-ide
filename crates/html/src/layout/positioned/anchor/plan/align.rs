//! Сдвиг и выравнивание в ячейке области: края IMCB, align-self, коробка CB.

use super::super::area::{Al, Tracks, default_al};
use super::super::{AnchorPlan, CB};
use super::axis::Edge;
use crate::style::computed::Align;
use crate::style::values::value::{Len, anchor_get};
use gpui::{Bounds, Pixels, Window, px};

impl AnchorPlan {
    /// Сдвиг по оси стороны `k` (0 top, 1 right, 2 bottom, 3 left) — без
    /// `position-area`: коробка стоит у края содержащего блока на нулевой
    /// вставке.
    pub(super) fn shift(&self, k: usize, own: Bounds<Pixels>) -> f32 {
        let Some(sp) = &self.sides[k] else { return 0.0 };
        let y_axis = k.is_multiple_of(2);
        let end_side = k == 1 || k == 2;
        // Край ПОЛЕЙ коробки по этой стороне.
        let own_edge = match k {
            0 => f32::from(own.origin.y) - sp.margin,
            1 => f32::from(own.origin.x) + f32::from(own.size.width) + sp.margin,
            2 => f32::from(own.origin.y) + f32::from(own.size.height) + sp.margin,
            _ => f32::from(own.origin.x) - sp.margin,
        };
        match self.resolve_anchor(sp.f.clone(), y_axis, end_side) {
            Edge::Abs(want) => want - own_edge,
            // Запасная длина в точках — вставка от края содержащего
            // блока; раскладка уже поставила коробку на нулевую вставку.
            Edge::FromEdge(v) => {
                if end_side {
                    -v
                } else {
                    v
                }
            }
            Edge::None => 0.0,
        }
    }

    /// Край inset-modified containing block со стороны `k`: край клетки,
    /// сдвинутый авторской вставкой (`auto` → 0; доля — от клетки, она и
    /// есть содержащий блок; `anchor()` — к краю якоря, §position-area).
    pub(super) fn imcb_edge(&self, k: usize, cell_edge: f32, cell_len: f32) -> f32 {
        let y_axis = k.is_multiple_of(2);
        let end_side = k == 1 || k == 2;
        let sign = if end_side { -1.0 } else { 1.0 };
        match self.inset[k] {
            Some(Len::Px(v)) => cell_edge + sign * v,
            Some(Len::Pct(p)) => cell_edge + sign * p * cell_len,
            Some(Len::Anchor(i)) => match anchor_get(i) {
                Some(f) if f.size.is_none() => match self.resolve_anchor(f, y_axis, end_side) {
                    Edge::Abs(w) => w,
                    Edge::FromEdge(v) => cell_edge + sign * v,
                    Edge::None => cell_edge,
                },
                _ => cell_edge,
            },
            _ => cell_edge,
        }
    }

    /// Выравнивание в оси, нужен ли сдвиг при переполнении и задано ли
    /// `safe` (§position-area-alignment; Blink `ComputeAlignment`): явное
    /// значение; иначе `normal` — к единственной не-`auto` вставке оси
    /// (unsafe), иначе умолчание области.
    pub(super) fn align_for(&self, x_axis: bool, t: Tracks) -> (Al, bool, bool) {
        // css-align: `align-self` — блочная ось СОДЕРЖАЩЕГО БЛОКА,
        // `justify-self` — строчная.
        let (explicit, safe) = if x_axis == self.cb_vertical {
            (self.align_self, self.safe_align)
        } else {
            (self.justify_self, self.safe_justify)
        };
        if let Some(a) = explicit {
            let al = match a {
                Align::Center => Al::Center,
                Align::AnchorCenter => Al::AnchorCenter,
                Align::End => Al::End,
                Align::Stretch => Al::Stretch,
                Align::Start | Align::Baseline => Al::Start,
            };
            return (al, true, safe);
        }
        let set = |l: Option<Len>| !matches!(l, None | Some(Len::Auto));
        let (s_set, e_set) = if x_axis {
            (set(self.inset[3]), set(self.inset[1]))
        } else {
            (set(self.inset[0]), set(self.inset[2]))
        };
        match (s_set, e_set) {
            (true, false) => (Al::Start, false, false),
            (false, true) => (Al::End, false, false),
            _ => (default_al(t), true, false),
        }
    }

    /// Рамка исходного содержащего блока: реестр `CB` по `cb_node`; корень
    /// и `fixed` — окно. Нет в реестре (блок не через `styled_div_with`,
    /// например строчный) — область не применяется.
    pub(super) fn cb_bounds(&self, window: &Window) -> Option<Bounds<Pixels>> {
        if self.fixed || self.cb_node == 0 {
            return Some(Bounds {
                origin: gpui::point(px(0.0), px(0.0)),
                size: window.viewport_size(),
            });
        }
        CB.with(|m| m.borrow().get(&self.cb_node).copied())
    }
}

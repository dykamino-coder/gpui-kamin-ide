//! Размещение по оси: вставки от якоря и свободная ось без якорной ссылки.

use super::super::AnchorPlan;
use super::super::area::{Al, Tracks};
use crate::style::computed::Align;
use gpui::{Bounds, Pixels};

impl AnchorPlan {
    /// Одна ось области: сетка → клетка → inset-modified containing block →
    /// выравнивание → зажим (`settle_axis`). Возвращает сдвиг границы
    /// коробки, длину IMCB (для растяжки и `position-try-order`) и признак
    /// переполнения IMCB коробкой полей.
    pub(super) fn axis_place(
        &self,
        x: bool,
        t: Tracks,
        a: Bounds<Pixels>,
        cb: Bounds<Pixels>,
        own: Bounds<Pixels>,
    ) -> (f32, f32, bool, f32) {
        let f = f32::from;
        let (cs, ce, as_, ae, os, olen) = if x {
            (
                f(cb.origin.x),
                f(cb.origin.x) + f(cb.size.width),
                f(a.origin.x),
                f(a.origin.x) + f(a.size.width),
                f(own.origin.x),
                f(own.size.width),
            )
        } else {
            (
                f(cb.origin.y),
                f(cb.origin.y) + f(cb.size.height),
                f(a.origin.y),
                f(a.origin.y) + f(a.size.height),
                f(own.origin.y),
                f(own.size.height),
            )
        };
        // §position-area-grid-resolution: якорь за краем содержащего блока
        // раздвигает крайние линии.
        let lines = [cs.min(as_), as_, ae, ce.max(ae)];
        let (s, e) = (lines[t.lo as usize], lines[t.hi as usize]);
        let (k_start, k_end) = if x { (3usize, 1usize) } else { (0, 2) };
        let is = self.imcb_edge(k_start, s, e - s);
        let ie = self.imcb_edge(k_end, e, e - s);
        let (m_s, m_e) = (self.margin[k_start], self.margin[k_end]);
        // Выравнивается коробка ПОЛЕЙ (`auto`-поля уже нули: `m` в `of`).
        let mbox = olen + m_s + m_e;
        let (al, shift_in, safe) = self.align_for(x, t);
        let pos = match al {
            Al::Start | Al::Stretch => is,
            Al::End => ie - mbox,
            Al::Center => (is + ie - mbox) / 2.0,
            Al::AnchorCenter => (as_ + ae - mbox) / 2.0,
        };
        let start_bias = !if x {
            self.cb_flipped.0
        } else {
            self.cb_flipped.1
        };
        let pos = settle_axis(
            pos,
            mbox,
            is,
            ie,
            cs,
            ce,
            shift_in,
            start_bias,
            safe,
            al == Al::AnchorCenter,
        );
        // Четвёртое — длина самой клетки `[s, e]`: база долей (`CELL_NOW`).
        (
            pos + m_s - os,
            (ie - is).max(0.0),
            overflows(pos, mbox, is, ie),
            (e - s).max(0.0),
        )
    }

    /// Ось без `position-area`: IMCB — содержащий блок, срезанный авторскими
    /// вставками (`auto` → край блока: для `anchor-center` так велит
    /// §anchor-center, для проверки переполнения — как Blink при `auto`-конце,
    /// «растёт к краю»); `anchor-center` центрирует по якорю и зажимает
    /// (`settle_axis`), иначе коробка стоит там, куда её довезли
    /// `anchor()`-вставки (`shift`). Без рамки содержащего блока — только сдвиг.
    pub(super) fn free_axis(
        &self,
        x: bool,
        cb: Option<Bounds<Pixels>>,
        own: Bounds<Pixels>,
    ) -> (f32, f32, bool) {
        let f = f32::from;
        let (k_start, k_end) = if x { (3usize, 1usize) } else { (0, 2) };
        let (os, olen) = if x {
            (f(own.origin.x), f(own.size.width))
        } else {
            (f(own.origin.y), f(own.size.height))
        };
        let (m_s, m_e) = (self.margin[k_start], self.margin[k_end]);
        let mbox = olen + m_s + m_e;
        // Без области по оси побеждает начальная сторона: `left` при
        // заданных `left` и `right`.
        let plain = if self.sides[k_start].is_some() {
            self.shift(k_start, own)
        } else {
            self.shift(k_end, own)
        };
        let Some(cb) = cb else {
            return (plain, 0.0, false);
        };
        let (cs, ce) = if x {
            (f(cb.origin.x), f(cb.origin.x) + f(cb.size.width))
        } else {
            (f(cb.origin.y), f(cb.origin.y) + f(cb.size.height))
        };
        let is = self.imcb_edge(k_start, cs, ce - cs);
        let ie = self.imcb_edge(k_end, ce, ce - cs);
        let (explicit, safe) = if x == self.cb_vertical {
            (self.align_self, self.safe_align)
        } else {
            (self.justify_self, self.safe_justify)
        };
        let anchor = if explicit == Some(Align::AnchorCenter) {
            self.lookup(None)
        } else {
            None
        };
        let pos = match anchor {
            Some(a) => {
                let (as_, ae) = if x {
                    (f(a.origin.x), f(a.origin.x) + f(a.size.width))
                } else {
                    (f(a.origin.y), f(a.origin.y) + f(a.size.height))
                };
                let start_bias = !if x {
                    self.cb_flipped.0
                } else {
                    self.cb_flipped.1
                };
                settle_axis(
                    (as_ + ae - mbox) / 2.0,
                    mbox,
                    is,
                    ie,
                    cs,
                    ce,
                    true,
                    start_bias,
                    safe,
                    true,
                )
            }
            None => os - m_s + plain,
        };
        (
            pos + m_s - os,
            (ie - is).max(0.0),
            overflows(pos, mbox, is, ie),
        )
    }
}

/// Куда просится край по `anchor()`.
// `FromEdge` names the anchor-relative inset, not a nested `Edge`.
#[allow(clippy::enum_variant_names)]
pub(super) enum Edge {
    Abs(f32),
    FromEdge(f32),
    None,
}

/// Зажим коробки полей по оси — хвост Blink `ComputeInsets`
/// (`absolute_utils.cc`): при `safe` переполнение прижимает к безопасному
/// краю (начало содержащего блока; у `anchor-center` — только когда центр по
/// якорю выходит за ЭТОТ край, `half_size = c − imcb_start`); иначе при
/// умолчальном переполнении css-align (`shift_in`) коробка сдвигается внутрь
/// IMCB, если в него влезает, иначе внутрь объединения IMCB с содержащим
/// блоком; при нехватке места побеждает край начала
/// (`adjust_end(); adjust_start()`).
#[allow(clippy::too_many_arguments)]
pub(super) fn settle_axis(
    pos: f32,
    mbox: f32,
    is: f32,
    ie: f32,
    cs: f32,
    ce: f32,
    shift_in: bool,
    start_bias: bool,
    safe: bool,
    anchor_center: bool,
) -> f32 {
    if safe {
        let over = if anchor_center {
            if start_bias {
                pos < is
            } else {
                pos + mbox > ie
            }
        } else {
            mbox > ie - is
        };
        return if !over {
            pos
        } else if start_bias {
            is
        } else {
            ie - mbox
        };
    }
    if !shift_in {
        return pos;
    }
    let (lo, hi) = if mbox <= ie - is {
        (is, ie - mbox)
    } else {
        (is.min(cs), ie.max(ce) - mbox)
    };
    if start_bias {
        pos.min(hi).max(lo)
    } else {
        pos.max(lo).min(hi)
    }
}

/// Переполняет ли коробка полей `[pos, pos+mbox]` IMCB `[is, ie]` (Blink
/// `CalculateNonOverflowingRangeInOneAxis`: любой край за краем IMCB).
pub(super) fn overflows(pos: f32, mbox: f32, is: f32, ie: f32) -> bool {
    pos < is - 0.01 || pos + mbox > ie + 0.01
}

//! Arithmetic length terms preserve unresolved percentage dependencies.

use super::{Len, calc_get, calc_store};
use crate::document::state::ValueCx;

/// Length arithmetic retains each unit until its resolution basis is available.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Sum {
    pub lh: f32,
    pub px: f32,
    pub pct: f32,
    /// Percentage terms retain their type after cancellation (CSS Values 4 §10.11).
    pub has_percentage: bool,
    pub em: f32,
    pub ch: f32,
    pub ex: f32,
    pub ic: f32,
    /// Высота прописной — единица `cap`. Своего варианта `Len` у неё нет:
    /// природа живёт только в сумме и сворачивается в точки там же, где
    /// `em`/`ch`/`ex`/`ic` (`Computed::resolve_em`).
    pub cap: f32,
    pub vh: f32,
    pub vw: f32,
    /// Единицы окна и контейнера без своего варианта `Len` (`vmin`, `vi`,
    /// `cqw`…) — индексы `super::units::Ext`, решаются в `fold_viewport`.
    pub ext: [f32; super::units::EXT_N],
}

impl Sum {
    pub(super) fn from_len(len: Len, vcx: ValueCx<'_>) -> Option<Self> {
        let mut s = Sum::default();
        match len {
            Len::Px(v) => s.px = v,
            Len::Pct(v) => {
                s.pct = v;
                s.has_percentage = true;
            }
            Len::Em(v) => s.em = v,
            Len::Ch(v) => s.ch = v,
            Len::Ic(v) => s.ic = v,
            Len::Ex(v) => s.ex = v,
            Len::Lh(v) => s.lh = v,
            Len::LhPx(l, p) => {
                s.lh = l;
                s.px = p;
            }
            Len::EmPx(e, p) => {
                s.em = e;
                s.px = p;
            }
            Len::Vh(v) => s.vh = v,
            Len::Vw(v) => s.vw = v,
            Len::Calc(i) => s = calc_get(i, vcx),
            // Якорная вставка в арифметику не входит: её довесок уже внутри
            // `AnchorFn::add`, а сама она решается на подготовке кадра.
            Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent | Len::Anchor(_) => {
                return None;
            }
        }
        Some(s)
    }

    pub(super) fn scaled(self, k: f32) -> Self {
        Sum {
            lh: self.lh * k,
            px: self.px * k,
            pct: self.pct * k,
            has_percentage: self.has_percentage,
            em: self.em * k,
            ch: self.ch * k,
            ic: self.ic * k,
            ex: self.ex * k,
            cap: self.cap * k,
            vh: self.vh * k,
            vw: self.vw * k,
            ext: self.ext.map(|v| v * k),
        }
    }

    pub(super) fn add(self, other: Self, sign: f32) -> Self {
        Sum {
            lh: self.lh + sign * other.lh,
            px: self.px + sign * other.px,
            pct: self.pct + sign * other.pct,
            has_percentage: self.has_percentage || other.has_percentage,
            em: self.em + sign * other.em,
            ch: self.ch + sign * other.ch,
            ic: self.ic + sign * other.ic,
            ex: self.ex + sign * other.ex,
            cap: self.cap + sign * other.cap,
            vh: self.vh + sign * other.vh,
            vw: self.vw + sign * other.vw,
            ext: std::array::from_fn(|i| self.ext[i] + sign * other.ext[i]),
        }
    }

    /// Единственная живая природа суммы: номер поля и величина. Пустая сумма —
    /// ноль в точках. Две и больше природ — `None`: такие доводы `min()`/`max()`
    /// сравнимы только на раскладке.
    pub(super) fn nature(self) -> Option<(u8, f32)> {
        let f = self.natures();
        let mut alive = f
            .iter()
            .enumerate()
            .filter(|(i, v)| **v != 0.0 || (*i == 1 && self.has_percentage));
        match (alive.next(), alive.next()) {
            (None, _) => Some((0, 0.0)),
            (Some((i, v)), None) => Some((i as u8, *v)),
            _ => None,
        }
    }

    /// Свёртка в длину: сокращение слагаемых учтено, поэтому
    /// `calc(100% + 6em + 50%*4 - 12em/2)` даёт чистые 300 % — `em` в нём
    /// взаимно уничтожаются.
    pub fn collapse(self, vcx: ValueCx<'_>) -> Option<Len> {
        // Even a cancelled percentage needs a definite basis. Dropping it turns
        // an auto block height into a length before CSS 2.1 §10.5 can apply.
        if self.has_percentage && self.pct == 0.0 {
            let length = Self {
                has_percentage: false,
                ..self
            };
            return Some(if length == Self::default() {
                Len::Pct(0.0)
            } else {
                Len::Calc(calc_store(self, vcx))
            });
        }
        // Живая `cap` своего варианта `Len` не имеет — сумма доживает
        // индексом и сворачивается в `resolve_em`, где известны семейство и
        // кегль. Ранний возврат, а НЕ правка веток ниже: те ветки замерены
        // (★ `gap-003-ltr` 0.00 → 4.12), и трогать их из-за новой природы
        // нельзя.
        // Доля в смеси с ними живёт только через `collapse_mixed`.
        if self.cap != 0.0 || self.ext.iter().any(|v| *v != 0.0) {
            return (self.pct == 0.0).then(|| Len::Calc(calc_store(self, vcx)));
        }
        let rel = [
            (self.pct, Len::Pct as fn(f32) -> Len),
            (self.em, Len::Em as fn(f32) -> Len),
            (self.ch, Len::Ch as fn(f32) -> Len),
            (self.ic, Len::Ic as fn(f32) -> Len),
            (self.ex, Len::Ex as fn(f32) -> Len),
            (self.vh, Len::Vh as fn(f32) -> Len),
            (self.vw, Len::Vw as fn(f32) -> Len),
        ];
        let mut alive = rel.iter().filter(|(v, _)| *v != 0.0);
        match (alive.next(), alive.next(), self.lh != 0.0) {
            (None, _, false) => Some(Len::Px(self.px)),
            (Some((v, unit)), None, false) if self.px == 0.0 => Some(unit(*v)),
            // Кегльная доля с довеском в точках: разрешится вместе с `em`
            // (text-shadow-orientation-upright-001: `calc(1em + 8px)`).
            (Some((v, unit)), None, false) if matches!(unit(*v), Len::Em(_)) => {
                Some(Len::EmPx(*v, self.px))
            }
            // Кратное строки с довеском в точках: разрешится при слиянии.
            (None, _, true) => Some(if self.px == 0.0 {
                Len::Lh(self.lh)
            } else {
                Len::LhPx(self.lh, self.px)
            }),
            // Font and viewport terms resolve in the cascade. Percentage mixtures
            // require the layout basis and are retained by collapse_mixed.
            _ if self.pct == 0.0 => Some(Len::Calc(calc_store(self, vcx))),
            _ => None,
        }
    }

    /// Свёртка, при которой процентная смесь ДОЖИВАЕТ индексом в арене —
    /// для потребителей с известным размером коробки (`Len::parse_mixed`).
    /// `collapse` отдаёт `None` ровно в одном случае — доля вместе с другой
    /// природой, — и только он сюда и попадает.
    pub fn collapse_mixed(self, vcx: ValueCx<'_>) -> Option<Len> {
        self.collapse(vcx)
            .or_else(|| Some(Len::Calc(calc_store(self, vcx))))
    }

    /// Return length-percentage terms, including an unresolved zero percentage.
    pub fn pct_px(self) -> Option<(f32, f32)> {
        (self.has_percentage || self.pct != 0.0)
            .then(|| self.length_percentage())
            .flatten()
    }

    /// Project resolved length-percentage terms without allocating or storing a handle.
    pub fn length_percentage(self) -> Option<(f32, f32)> {
        let rest = Sum {
            px: 0.0,
            pct: 0.0,
            has_percentage: false,
            ..self
        };
        (rest == Sum::default()).then_some((self.pct, self.px))
    }

    /// Свёртка для межбуквенного и межсловного интервала: там и доля, и `em`
    /// считаются от кегля, поэтому смешанное `calc(400% + 1em)` складывается
    /// вместо того чтобы пропасть.
    pub(super) fn collapse_spacing(self, vcx: ValueCx<'_>) -> Option<Len> {
        Sum {
            pct: 0.0,
            has_percentage: false,
            em: self.em + self.pct,
            ..self
        }
        .collapse(vcx)
    }
}

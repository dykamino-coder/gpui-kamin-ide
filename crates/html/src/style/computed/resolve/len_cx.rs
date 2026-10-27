//! Канонический перевод `Len` в точки с ЯВНЫМ контекстом: база долей, шрифт и
//! окно. Чего в контексте нет, то не решается (`None`), а что делать с
//! нерешённым, вызывающий выбирает по имени (`px_strict`, `px_lossy_zero`) —
//! те же контракты, что у `len_contract`, и при пустом контексте они
//! совпадают с ними.
//!
//! Единицы окна (css-values-4 §6.1.2): `vw`/`vh` — свои варианты `Len`,
//! `vmin`/`vmax`/`vi`/`vb` и единицы контейнера живут слагаемыми `calc` и
//! сворачиваются `Sum::fold_viewport`. Малое, большое и динамическое окна
//! совпадают (`style::values::value::units`).

use super::len_px::FontCx;
use crate::document::state::ValueCx;
use crate::style::computed::Computed;
use crate::style::values::value::{Len, calc_get};

/// Окно для единиц окна: размер (уже без полос прокрутки), ось письма самой
/// коробки (для `vi`/`vb`/`cqi`/`cqb`) и содержимое ближайшего контейнера
/// запросов размера (без него — окно, css-conditional-5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewportCx {
    pub size: (f32, f32),
    pub vertical: bool,
    pub container: Option<(f32, f32)>,
}

/// Контекст перевода длины в точки. `None` в поле — эта природа единиц в
/// данном месте не решается.
#[derive(Clone, Copy, Debug, Default)]
pub struct LenCx<'a> {
    /// База долей.
    pub basis: Option<f32>,
    /// Шрифт для `em`/`ex`/`ch`/`ic`/`lh`.
    pub font: Option<FontCx<'a>>,
    /// Окно для `vw`/`vh` и прочих единиц окна.
    pub viewport: Option<ViewportCx>,
}

impl ViewportCx {
    /// Окно заданного размера для коробки со стилем `style`: ось её письма и
    /// её контейнер запросов размера (недостающая ось — окно). Полосы
    /// прокрутки не вычитаются: размер уже окончательный.
    pub fn for_box(size: (f32, f32), style: &Computed) -> Self {
        // `zoom` масштабирует длины коробки, включая единицы окна
        // (css-viewport-1 §zoom-property). Вход контейнера — размер в CSS px
        // без его зума (Blink AdjustForAbsoluteZoom); здесь применяется зум
        // потребителя, как для остальных его длин.
        let zoom = style.zoom_eff.unwrap_or(1.0);
        let size = (size.0 * zoom, size.1 * zoom);
        ViewportCx {
            size,
            vertical: style.wm().vertical(),
            container: Some((
                style.cq_box.0.map_or(size.0, |v| v * zoom),
                style.cq_box.1.map_or(size.1, |v| v * zoom),
            )),
        }
    }
}

impl<'a> LenCx<'a> {
    /// Только окно коробки со стилем `style` (`ViewportCx::for_box`): доли и
    /// шрифтовые единицы не решаются.
    pub fn box_viewport(viewport: Option<(f32, f32)>, style: &Computed) -> Self {
        LenCx {
            viewport: viewport.map(|size| ViewportCx::for_box(size, style)),
            ..LenCx::default()
        }
    }

    /// Длина в точках; `None` — не решается в этом контексте (нет базы,
    /// шрифта или окна) либо это не длина (`auto`, размеры по содержимому,
    /// `anchor()`).
    pub fn px(self, l: Len, vcx: ValueCx<'_>) -> Option<f32> {
        let pct = |k: f32| self.basis.map(|b| b * k);
        Some(match l {
            Len::Px(v) => v,
            Len::Pct(k) => pct(k)?,
            Len::Calc(i) => {
                // Слагаемые окна и контейнера сворачиваются по окну
                // контекста; после свёртки сумма из одних точек — длина.
                let mut sum = calc_get(i, vcx);
                let folded = self
                    .viewport
                    .is_some_and(|vp| sum.fold_viewport(vp.size, vp.vertical, vp.container));
                let (k, add) = if folded {
                    sum.length_percentage()?
                } else {
                    sum.pct_px()?
                };
                if k == 0.0 { add } else { pct(k)? + add }
            }
            Len::Vw(k) => k * self.viewport?.size.0,
            Len::Vh(k) => k * self.viewport?.size.1,
            Len::Em(_)
            | Len::EmPx(..)
            | Len::Ch(_)
            | Len::Ex(_)
            | Len::Ic(_)
            | Len::Lh(_)
            | Len::LhPx(..) => font_px(l, self.font?, vcx),
            Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent | Len::Anchor(_) => {
                return None;
            }
        })
    }

    /// Только решённые длины: отсутствие значения и всё нерешённое — `None`.
    pub fn px_strict(self, l: Option<Len>, vcx: ValueCx<'_>) -> Option<f32> {
        self.px(l?, vcx)
    }

    /// Отсутствие значения — ноль, нерешённое (включая `auto`) —
    /// НЕПРОХОДИМО (`None`).
    pub fn px_missing_zero(self, l: Option<Len>, vcx: ValueCx<'_>) -> Option<f32> {
        match l {
            None => Some(0.0),
            Some(l) => self.px(l, vcx),
        }
    }

    /// Точки или ноль С ПОТЕРЕЙ: отсутствие значения и всё нерешённое —
    /// ноль. Законно там, где нерешённое по смыслу ничего не добавляет.
    pub fn px_lossy_zero(self, l: Option<Len>, vcx: ValueCx<'_>) -> f32 {
        self.px_strict(l, vcx).unwrap_or(0.0)
    }
}

/// Шрифтовые единицы по контексту шрифта.
fn font_px(l: Len, font: FontCx<'_>, vcx: ValueCx<'_>) -> f32 {
    let lh = || font.line_px.unwrap_or(1.2 * font.font_px);
    match l {
        Len::Em(k) => k * font.font_px,
        Len::EmPx(k, add) => k * font.font_px + add,
        Len::Ch(k) => {
            k * font.units.map_or_else(
                || crate::style::font_metrics::horizontal(font.family, font.metric_px, vcx).0,
                |u| u[1],
            )
        }
        Len::Ex(k) => {
            k * font.units.map_or_else(
                || crate::style::font_metrics::horizontal(font.family, font.metric_px, vcx).1,
                |u| u[2],
            )
        }
        Len::Ic(k) => {
            k * font.units.map_or_else(
                || crate::style::font_metrics::ideographic(font.family, font.metric_px, vcx),
                |u| u[3],
            )
        }
        Len::Lh(k) => k * lh(),
        Len::LhPx(k, add) => k * lh() + add,
        _ => 0.0,
    }
}

impl Len {
    /// Единицы контейнера становятся абсолютными длинами до наследования.
    pub(crate) fn fold_container(self, vp: ViewportCx, vcx: ValueCx<'_>) -> Len {
        if let Len::Calc(i) = self {
            let mut sum = calc_get(i, vcx);
            if sum.fold_container(vp.container.unwrap_or(vp.size), vp.vertical) {
                return sum.collapse_mixed(vcx).unwrap_or(self);
            }
        }
        self
    }

    /// Шаг каскада «решить единицы окна»: `vw`/`vh` — в точки, `calc` с
    /// единицами окна или контейнера — свёрнутая сумма (смесь с долей
    /// доживает индексом арены). Без слагаемых окна длина не меняется: арена
    /// append-only, а `collapse` стёр бы процентную смесь.
    pub fn fold_viewport(self, vp: ViewportCx, vcx: ValueCx<'_>) -> Len {
        match self {
            Len::Vw(k) => Len::Px(k * vp.size.0),
            Len::Vh(k) => Len::Px(k * vp.size.1),
            Len::Calc(i) => {
                let mut sum = calc_get(i, vcx);
                if sum.fold_viewport(vp.size, vp.vertical, vp.container) {
                    sum.collapse_mixed(vcx).unwrap_or(self)
                } else {
                    self
                }
            }
            other => other,
        }
    }
}

impl Computed {
    /// Окно единиц окна для этой коробки: размер без полос прокрутки
    /// (`viewport_unit_insets`), ось её письма, контейнер запросов размера
    /// (недостающая ось — окно).
    pub fn viewport_cx(&self, raw: (f32, f32)) -> ViewportCx {
        let insets = self.viewport_unit_insets.unwrap_or_default();
        let size = ((raw.0 - insets[0]).max(0.0), (raw.1 - insets[1]).max(0.0));
        ViewportCx::for_box(size, self)
    }
}

#[cfg(test)]
#[path = "len_cx_tests.rs"]
mod tests;

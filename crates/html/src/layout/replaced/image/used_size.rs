//! Входы используемого размера картинки замещаемого (CSS 2.1 §10.3.2, §10.4, §10.6.2):
//! стиль элемента, природные размеры источника и ширина содержащего блока, поправка
//! `box-sizing: border-box`, пределы в точках содержимого и соотношение сторон. Решение
//! о размере рисунка (`Picture`) — в `sized`/`auto_sized`; рисунок по нему строит покраска.

use crate::layout::replaced::natural::Intrinsic;
use crate::layout::replaced::replaced_used_style;
use crate::style::computed::Computed;
use crate::style::values::value::{Len, px_lossy_zero};

/// Размеры рисунка: что из них поставить картинке и во что растрировать вектор.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Picture {
    /// Ни одна заданная сторона не меняет рисунок: свой размер источника.
    Natural,
    /// Обе стороны заданы в точках: растр размера `raster`, рисунок — `size`, а у
    /// элемента гибкого контейнера (`None`) — во всю коробку.
    Authored {
        raster: (f32, f32),
        size: Option<(f32, f32)>,
    },
    /// Используемый размер `size` с `object-fit: fill`, растр — `raster`.
    Used {
        raster: (f32, f32),
        size: (f32, f32),
    },
    /// Доля ширины при авто-высоте: во всю ширину коробки, высота — из соотношения.
    FullWidth { ratio: Option<f32> },
    /// Доля высоты при авто-ширине: во всю высоту коробки.
    FullHeight { ratio: Option<f32> },
    /// Обе стороны заданы, не обе в точках: доля ширины и высота в точках.
    FullWidthPxHeight(f32),
    /// Ширина в точках и доля высоты.
    PxWidthFullHeight(f32),
    /// Иные заданные стороны: рисунок во всю коробку.
    Full,
}

/// Входы размера одного рисунка.
pub(crate) struct PictureSizing<'a> {
    pub(super) style: &'a Computed,
    /// Природные размеры источника; `None` — источника нет.
    pub(super) natural: Option<Intrinsic>,
    pub(super) cb_width: Option<f32>,
    /// Контентная поправка: при `box-sizing: border-box` названный размер или
    /// предел включает паддинг и рамку — рисунку остаётся остальное.
    pub(crate) sub: (f32, f32),
    pub(super) max_w: Option<f32>,
    pub(super) max_h: Option<f32>,
}

impl<'a> PictureSizing<'a> {
    pub(crate) fn new(
        style: &'a Computed,
        natural: Option<Intrinsic>,
        cb_width: Option<f32>,
    ) -> Self {
        let side = px_lossy_zero;
        let sub = if style.border_box == Some(true) {
            let b = style.borders();
            (
                side(style.padding.left) + side(style.padding.right) + side(b.left) + side(b.right),
                side(style.padding.top) + side(style.padding.bottom) + side(b.top) + side(b.bottom),
            )
        } else {
            (0.0, 0.0)
        };
        let mut sizing = PictureSizing {
            style,
            natural,
            cb_width,
            sub,
            max_w: None,
            max_h: None,
        };
        sizing.max_w = sizing.clamp(style.max_width, sub.0);
        sizing.max_h = sizing.clamp(style.max_height, sub.1);
        sizing
    }

    /// Предел в точках содержимого: заданный в точках минус поправка.
    pub(super) fn clamp(&self, l: Option<Len>, sub: f32) -> Option<f32> {
        match l {
            Some(Len::Px(v)) => Some((v - sub).max(0.0)),
            _ => None,
        }
    }

    /// Сперва потолок, затем пол (§10.4).
    pub(super) fn limit(&self, v: f32, min: Option<f32>, max: Option<f32>) -> f32 {
        let v = match max {
            Some(m) => v.min(m),
            None => v,
        };
        match min {
            Some(m) => v.max(m),
            None => v,
        }
    }

    fn natural_ratio(&self) -> Option<f32> {
        self.natural.and_then(|i| {
            i.ratio.or(match (i.w, i.h) {
                (Some(w), Some(h)) if h > 0.0 => Some(w / h),
                _ => None,
            })
        })
    }

    /// Соотношение сторон рисунка.
    pub(crate) fn ratio(&self) -> Option<f32> {
        // CSS Sizing 4 #aspect-ratio: authored ratio overrides the natural ratio.
        if let Some(r) = self.style.aspect_ratio.filter(|r| *r > 0.0) {
            return Some(r);
        }
        // `auto <ratio>`: природное сильнее, заявленное — запасное.
        self.natural_ratio()
            .filter(|r| *r > 0.0)
            .or(self.style.aspect_ratio_auto.filter(|r| *r > 0.0))
    }

    pub(super) fn transfer(&self, size: f32, ratio: f32, from_width: bool) -> f32 {
        replaced_used_style::transfer(
            self.style,
            size,
            ratio,
            from_width,
            [self.sub.0, self.sub.1],
        )
    }
}

#[cfg(test)]
mod tests;

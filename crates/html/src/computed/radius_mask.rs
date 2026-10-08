//! Select the raster corner mask when circular quad radii cannot express the CSS shape.

use super::{Computed, Overflow};
use crate::value::Len;

impl Computed {
    /// Use the group mask for elliptical, shaped, or oversized nonuniform corners.
    pub fn radius_masked(&self) -> bool {
        // CSS Backgrounds 3 §5.1: each percentage resolves against its own
        // border-box axis. A non-square box therefore needs elliptical corners;
        // the GPUI quad only supports one circular radius per corner.
        let percentage = [
            self.radius.tl,
            self.radius.tr,
            self.radius.br,
            self.radius.bl,
        ]
        .iter()
        .any(|r| matches!(r, Some(Len::Pct(p)) if *p > 0.0));
        let square = match (self.width, self.height) {
            (Some(Len::Px(w)), Some(Len::Px(h))) => {
                let sides = |s: super::Sides| {
                    [s.top, s.right, s.bottom, s.left].map(|v| match v {
                        Some(Len::Px(v)) => v,
                        _ => 0.0,
                    })
                };
                let [pt, pr, pb, pl] = sides(self.padding);
                let [bt, br, bb, bl] = sides(self.borders());
                let extra = if self.border_box == Some(true) {
                    0.0
                } else {
                    pl + pr + bl + br - pt - pb - bt - bb
                };
                (w - h + extra).abs() < 0.001
            }
            _ => false,
        };
        if self.radius_ell.is_some() || self.corner_shaped() || (percentage && !square) {
            return true;
        }
        // `contain: paint` со скруглением: обрезка содержимого обязана учесть
        // углы (css-contain-2 §3.3: «clipped to the overflow clip edge … taking
        // corner clipping into account»). Маска gpui — только прямоугольник
        // (`ContentMask`), и `overflow_hidden` оставлял переполнение в углах
        // (`contain-paint-001`: красная полоса за кругом). Маска группы по
        // `rrect` режет и фон, и детей; без рамки padding-box = border-box, и
        // край маски — ровно край обрезки. Тень и контур лежат ВНЕ коробки —
        // маска их съела бы, такие коробки идут прежним путём.
        // Решение обязано СОВПАСТЬ на двух стилях одной коробки: `apply_radius`
        // читает слитый (`inline::inherit` → `resolve_em`, радиус уже в
        // точках), а `render::grouped` — собственный `e.style`, где `4em`
        // доживает как `Len::Em`. Гейт по одним точкам и долям снимал
        // скругление квада, а маски на `e.style` не заводил — зелёный квадрат
        // без обрезки углов (`contain-paint-clip-002`: `border-radius: 4em`,
        // 0.00 → 0.65 = площадь углов 120² − π·60²). Blink решает по одному
        // стилю (`paint_property_tree_builder.cc:3010-3012`,
        // `NeedsInnerBorderRadiusClip`). Для гейта важен лишь знак длины —
        // меряем единой точкой, как `apply::radius_px`. Тень в единицах шрифта
        // до `resolve_em` лежит строкой (`shadow_raw`) — исключается и она.
        // Внутренняя копия прокрутки (`scroller`) идёт мимо `grouped`: маски
        // там нет, снимать скругление квада нельзя.
        let font_len = |l: Option<Len>| match l {
            Some(Len::Pct(p)) => Some(p),
            Some(l) => crate::metrics::fallback_len_px(l, "", 16.0),
            None => None,
        };
        if self.contain_paint == Some(true)
            && self.shadows.is_empty()
            && self.shadow_raw.is_none()
            && self.outline.is_none()
            && !self.scroller
            && self.overflow_x != Some(Overflow::Scroll)
            && self.overflow_y != Some(Overflow::Scroll)
        {
            let rounded = |l: Option<Len>| font_len(l).is_some_and(|v| v > 0.0);
            let bare = |l: Option<Len>| font_len(l).is_none_or(|v| v <= 0.0);
            let b = self.borders();
            if [
                self.radius.tl,
                self.radius.tr,
                self.radius.br,
                self.radius.bl,
            ]
            .into_iter()
            .any(rounded)
                && bare(b.top)
                && bare(b.right)
                && bare(b.bottom)
                && bare(b.left)
            {
                return true;
            }
        }
        let side = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let round = [
            side(self.radius.tl),
            side(self.radius.tr),
            side(self.radius.br),
            side(self.radius.bl),
        ];
        let (w, h) = (side(self.width), side(self.height));
        let max_r = round.iter().cloned().fold(0.0f32, f32::max);
        let uniform = round.iter().all(|r| (r - round[0]).abs() < 0.01);
        w > 0.0 && h > 0.0 && !uniform && max_r > w.min(h) * 0.5 + 0.01
    }
}

//! Вычисленный стиль: когда скругление углов нельзя выразить круглыми
//! радиусами квада GPUI и нужна растровая маска группы (`radius_masked`) —
//! эллиптические углы (процент по своей оси, CSS Backgrounds 3 §5.1; смешанный
//! `calc`), `corner-shape`, неравномерные радиусы больше половины коробки и
//! `contain: paint` со скруглением без рамки (css-contain-2 §3.3).

use super::{Computed, Overflow};
use crate::style::computed::FontCx;
use crate::style::values::value::{Len, px_lossy_zero};
use crate::values::ValueCx;

impl Computed {
    /// Use the group mask for elliptical, shaped, or oversized nonuniform corners.
    pub fn radius_masked(&self, vcx: ValueCx<'_>) -> bool {
        self.radius_masked_with(
            crate::style::computed::single_corner::eligible(self),
            self.reset_effects.shadows.is_empty(),
            vcx,
        )
    }

    pub fn radius_masked_with(
        &self,
        single_corner: bool,
        shadows_empty: bool,
        vcx: ValueCx<'_>,
    ) -> bool {
        if single_corner {
            return false;
        }
        // CSS Backgrounds 3 §5.1: each percentage resolves against its own
        // border-box axis. A non-square box therefore needs elliptical corners;
        // the GPUI quad only supports one circular radius per corner.
        let radii = [
            self.reset_border.radius.tl,
            self.reset_border.radius.tr,
            self.reset_border.radius.br,
            self.reset_border.radius.bl,
        ];
        // A mixed length-percentage cannot be resolved by the quad, even when
        // the two box axes happen to be equal.
        let calculated = radii.iter().any(|r| match r {
            Some(Len::Calc(i)) => {
                let sum = crate::style::values::value::calc_get(*i, vcx);
                sum.has_percentage || sum.pct != 0.0
            }
            _ => false,
        });
        let percentage = radii
            .iter()
            .any(|r| matches!(r, Some(Len::Pct(p)) if *p > 0.0));
        let square = match (self.reset_box_layout.width, self.reset_box_layout.height) {
            (Some(Len::Px(w)), Some(Len::Px(h))) => {
                let sides = |s: super::Sides| [s.top, s.right, s.bottom, s.left].map(px_lossy_zero);
                let [pt, pr, pb, pl] = sides(self.reset_box_layout.padding);
                let [bt, br, bb, bl] = sides(self.borders());
                let extra = if self.reset_border.border_box == Some(true) {
                    0.0
                } else {
                    pl + pr + bl + br - pt - pb - bt - bb
                };
                (w - h + extra).abs() < 0.001
            }
            _ => false,
        };
        if self.reset_border.radius_ell.is_some()
            || self.corner_shaped()
            || calculated
            || (percentage && !square)
        {
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
        // читает слитый (`style::cascade::inherit` → `resolve_em`, радиус уже в
        // точках), а `render::grouped` — собственный `e.style`, где `4em`
        // доживает как `Len::Em`. Гейт по одним точкам и долям снимал
        // скругление квада, а маски на `e.style` не заводил — зелёный квадрат
        // без обрезки углов (`contain-paint-clip-002`: `border-radius: 4em`,
        // расхождение = площадь углов 120² − π·60²). Blink решает по одному
        // стилю (`paint_property_tree_builder.cc:3010-3012`,
        // `NeedsInnerBorderRadiusClip`). Для гейта важен лишь знак длины —
        // меряем единой точкой, как `apply::radius_px`. Тень в единицах шрифта
        // до `resolve_em` лежит строкой (`shadow_raw`) — исключается и она.
        // Внутренняя копия прокрутки (`scroller`) идёт мимо `grouped`: маски
        // там нет, снимать скругление квада нельзя.
        let font_len = |l: Option<Len>| match l {
            Some(Len::Pct(p)) => Some(p),
            Some(l) => l.to_px(None, FontCx::FALLBACK, vcx),
            None => None,
        };
        if self.reset_context.contain_paint == Some(true)
            && shadows_empty
            && self.reset_effects.shadow_raw.is_none()
            && self.reset_border.outline.is_none()
            && !self.reset_context.scroller
            && self.reset_box_layout.overflow_x != Some(Overflow::Scroll)
            && self.reset_box_layout.overflow_y != Some(Overflow::Scroll)
        {
            let rounded = |l: Option<Len>| font_len(l).is_some_and(|v| v > 0.0);
            let bare = |l: Option<Len>| font_len(l).is_none_or(|v| v <= 0.0);
            let b = self.borders();
            if [
                self.reset_border.radius.tl,
                self.reset_border.radius.tr,
                self.reset_border.radius.br,
                self.reset_border.radius.bl,
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
        let side = px_lossy_zero;
        let round = [
            side(self.reset_border.radius.tl),
            side(self.reset_border.radius.tr),
            side(self.reset_border.radius.br),
            side(self.reset_border.radius.bl),
        ];
        let (w, h) = (
            side(self.reset_box_layout.width),
            side(self.reset_box_layout.height),
        );
        let max_r = round.iter().cloned().fold(0.0f32, f32::max);
        let uniform = round.iter().all(|r| (r - round[0]).abs() < 0.01);
        w > 0.0 && h > 0.0 && !uniform && max_r > w.min(h) * 0.5 + 0.01
    }
}

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
    /// Use the group mask for shaped corners and corners that clip content.
    pub fn radius_masked(&self, vcx: ValueCx<'_>) -> bool {
        self.radius_masked_with(
            crate::style::computed::single_corner::eligible(self),
            self.reset_effects.shadows.is_empty(),
            vcx,
        )
    }

    /// The quad draws elliptical corners itself (`gpui::EllipseRadii`).
    pub fn radius_elliptic(&self, vcx: ValueCx<'_>) -> bool {
        self.radius_elliptic_with(
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
        self.radius_beyond_circles(single_corner, shadows_empty, vcx) && !self.ellipse_quad_draws()
    }

    pub fn radius_elliptic_with(
        &self,
        single_corner: bool,
        shadows_empty: bool,
        vcx: ValueCx<'_>,
    ) -> bool {
        self.radius_beyond_circles(single_corner, shadows_empty, vcx) && self.ellipse_quad_draws()
    }

    /// Corners a quad with quarter-ellipse corners draws (css-backgrounds-3
    /// §5.1, §5.5): elliptical, percentage or overlapping radii of a box
    /// whose corners shape only its own background, border and shadows (no
    /// content clip, which the group mask still cuts), with a solid border of
    /// one colour or none, plain `round` corners and lengths the quad
    /// resolves (no `calc()` mixing a percentage). Blink draws these as one
    /// elliptical `SkRRect`; a group with a mask is an offscreen surface per
    /// box (99.8 % of the groups of a page drawn in CSS).
    fn ellipse_quad_draws(&self) -> bool {
        self.ellipse_quad_refusal().is_none()
    }

    /// Why a quad does not draw these corners (`ellipse_quad_draws`), named
    /// for the DevTools journal: `corner_shape`, `calc`, `border_shape`,
    /// `border_image`, `border_style`, `border_colours`, `clip` or
    /// `inset_shadow`.
    pub fn ellipse_quad_refusal(&self) -> Option<&'static str> {
        let b = &self.reset_border;
        let visible = |o: Option<Overflow>| matches!(o, None | Some(Overflow::Visible));
        // Solid, or `double` on every painted side at least 3px wide (two
        // lines and a gap, `apply::double_border`); narrower double sides
        // paint solid there and here alike.
        let widths = {
            let w = self.borders();
            [w.top, w.right, w.bottom, w.left].map(px_lossy_zero)
        };
        let styles = b.border_side_styles;
        let doubled = (0..4).any(|i| widths[i] >= 3.0 && styles[i] == Some(10));
        let double_whole =
            (0..4).all(|i| widths[i] <= 0.0 || (styles[i] == Some(10) && widths[i] >= 3.0));
        let solid = styles
            .iter()
            .all(|s| matches!(s, None | Some(0 | 1 | 9 | 10)))
            && (!doubled || double_whole);
        let one_colour = b.border_colors.iter().all(|c| *c == b.border_colors[0]);
        let calc = |l: &Len| matches!(l, Len::Calc(_));
        let calculated = [b.radius.tl, b.radius.tr, b.radius.br, b.radius.bl]
            .iter()
            .flatten()
            .any(calc)
            || b.radius_ell
                .iter()
                .flatten()
                .flatten()
                .any(|(x, y)| calc(x) || calc(y));
        let clips = self.reset_context.contain_paint == Some(true)
            || self.reset_context.scroller
            || !visible(self.reset_box_layout.overflow_x)
            || !visible(self.reset_box_layout.overflow_y);
        let patterned = b.border_dashed == Some(true) || b.border_dotted == Some(true) || !solid;
        // An inner shadow lies inside the padding edge, its outer edge the
        // padding box's elliptical corners (css-backgrounds-3 §7.1); its ring
        // (`decorations::inset_shadows`) is a circular quad that only the
        // group mask cuts to those ellipses. One still unresolved in font
        // units waits in `shadow_raw`.
        let inset_shadow = !self.reset_effects.inset_shadows.is_empty()
            || self
                .reset_effects
                .shadow_raw
                .as_deref()
                .is_some_and(|s| s.to_ascii_lowercase().contains("inset"));
        [
            (self.corner_shaped(), "corner_shape"),
            (calculated, "calc"),
            (b.border_shape.is_some(), "border_shape"),
            (b.border_image.is_some(), "border_image"),
            (patterned, "border_style"),
            (doubled && !one_colour, "border_colours"),
            (clips, "clip"),
            (inset_shadow, "inset_shadow"),
        ]
        .into_iter()
        .find_map(|(refused, why)| refused.then_some(why))
    }

    fn radius_beyond_circles(
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

#[cfg(test)]
#[path = "radius_mask_tests.rs"]
mod tests;

//! Общий обход длин в единицах окна и контейнера.
//!
//! Длины сворачиваются в копиях, а группа пишется, только если значение изменилось:
//! общая группа без единиц окна остаётся общей (Blink `ComputedStyle` setters сравнивают
//! перед записью, `computed_style.h` `SetBorderImageOutset`).
use super::*;

/// The lengths of `now` after `fix`, in the same order; `None` when none changed.
pub(super) fn fixed<const N: usize>(
    fix: impl Fn(&mut Option<Len>),
    now: [Option<Len>; N],
) -> Option<[Option<Len>; N]> {
    let next = now.map(|mut value| {
        fix(&mut value);
        value
    });
    (next != now).then_some(next)
}

fn sides_of(s: &Sides) -> [Option<Len>; 4] {
    [s.top, s.right, s.bottom, s.left]
}

fn set_sides(s: &mut Sides, [top, right, bottom, left]: [Option<Len>; 4]) {
    *s = Sides {
        top,
        right,
        bottom,
        left,
    };
}

impl Computed {
    pub(super) fn resolve_unit_lengths(
        &mut self,
        fix: impl Fn(&mut Option<Len>) + Copy,
        vcx: ValueCx<'_>,
    ) {
        // Тот же порядок свёртки, что и прежде: `calc` пишет в арену кадра по порядку.
        let layout = &*self.reset_box_layout;
        let sizes = fixed(
            fix,
            [
                layout.width,
                layout.height,
                layout.min_width,
                layout.min_height,
                layout.max_width,
                layout.max_height,
            ],
        );
        let basis = fixed(fix, [self.reset_flex.flex_basis]);
        let path = fixed(fix, [self.reset_svg_math.svg_path_length]);
        // Кегль — в общей группе наследуемых: правится, только если он в
        // единицах окна или контейнера.
        if let Some(size) = self.inh.font.font_size {
            let mut folded = Some(size);
            fix(&mut folded);
            if folded != Some(size) {
                self.inh.font.font_size = folded;
            }
        }
        let layout = &*self.reset_box_layout;
        let padding = fixed(fix, sides_of(&layout.padding));
        let snap_padding = fixed(fix, sides_of(&layout.snap.padding));
        let snap_margin = fixed(fix, sides_of(&layout.snap.margin));
        let margin = fixed(fix, sides_of(&layout.margin));
        if sizes.is_some()
            || padding.is_some()
            || snap_padding.is_some()
            || snap_margin.is_some()
            || margin.is_some()
        {
            let layout = &mut *self.reset_box_layout;
            if let Some([w, h, min_w, min_h, max_w, max_h]) = sizes {
                (layout.width, layout.height) = (w, h);
                (layout.min_width, layout.min_height) = (min_w, min_h);
                (layout.max_width, layout.max_height) = (max_w, max_h);
            }
            for (side, value) in [
                (&mut layout.padding, padding),
                (&mut layout.snap.padding, snap_padding),
                (&mut layout.snap.margin, snap_margin),
                (&mut layout.margin, margin),
            ] {
                if let Some(value) = value {
                    set_sides(side, value);
                }
            }
        }
        if let Some([basis]) = basis {
            self.reset_flex.flex_basis = basis;
        }
        if let Some([path]) = path {
            self.reset_svg_math.svg_path_length = path;
        }
        // Толщина рамки в единицах окна (`border-bottom: 50vh solid`,
        // `monolithic-overflow-021`): без перевода рамка выходила нулевой.
        if let Some(width) = fixed(fix, sides_of(&self.reset_border.border_width)) {
            set_sides(&mut self.reset_border.border_width, width);
        }
        if let Some(inset) = fixed(fix, sides_of(&self.reset_positioned.inset)) {
            set_sides(&mut self.reset_positioned.inset, inset);
        }
        self.resolve_radius_lengths(fix);
        if [
            self.reset_background.gradient.as_ref(),
            self.reset_background.gradient_current.as_ref(),
        ]
        .into_iter()
        .flatten()
        .any(Gradient::has_lengths)
        {
            for gradient in {
                let group = &mut *self.reset_background;
                [group.gradient.as_mut(), group.gradient_current.as_mut()]
            }
            .into_iter()
            .flatten()
            {
                gradient.resolve_lengths(fix, vcx);
            }
        }
        if !self.reset_background.bg_lists.is_empty() {
            self.reset_background.bg_lists.resolve_lengths(&fix, vcx);
        }
        if self.image_operands_ref().any(ImageSource::has_lengths) {
            for image in self.image_operands() {
                image.resolve_lengths(&fix, vcx);
            }
        }
        if let Some((row, col)) = self.reset_flex.gap
            && let Some([row, col]) = fixed(fix, [row, col])
        {
            self.reset_flex.gap = Some((row, col));
        }
    }
}

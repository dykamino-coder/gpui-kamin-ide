//! Общий обход длин в единицах окна и контейнера.
use super::*;

impl Computed {
    pub(super) fn resolve_unit_lengths(
        &mut self,
        fix: impl Fn(&mut Option<Len>) + Copy,
        vcx: ValueCx<'_>,
    ) {
        let sides = |s: &mut Sides| {
            for one in [&mut s.top, &mut s.right, &mut s.bottom, &mut s.left] {
                fix(one);
            }
        };
        for l in {
            let group_0 = &mut *self.reset_box_layout;
            [
                &mut group_0.width,
                &mut group_0.height,
                &mut group_0.min_width,
                &mut group_0.min_height,
                &mut group_0.max_width,
                &mut group_0.max_height,
                &mut self.reset_flex.flex_basis,
                &mut self.reset_svg_math.svg_path_length,
            ]
        } {
            fix(l);
        }
        // Кегль — в общей группе наследуемых: правится, только если он в
        // единицах окна или контейнера.
        if let Some(size) = self.inh.font.font_size {
            let mut folded = Some(size);
            fix(&mut folded);
            if folded != Some(size) {
                self.inh.font.font_size = folded;
            }
        }
        sides(&mut self.reset_box_layout.padding);
        sides(&mut self.reset_box_layout.snap.padding);
        sides(&mut self.reset_box_layout.snap.margin);
        sides(&mut self.reset_box_layout.margin);
        // Толщина рамки в единицах окна (`border-bottom: 50vh solid`,
        // `monolithic-overflow-021`): без перевода рамка выходила нулевой.
        sides(&mut self.reset_border.border_width);
        sides(&mut self.reset_positioned.inset);
        self.resolve_radius_lengths(fix);
        for gradient in {
            let group = &mut *self.reset_background;
            [group.gradient.as_mut(), group.gradient_current.as_mut()]
        }
        .into_iter()
        .flatten()
        {
            gradient.resolve_lengths(fix, vcx);
        }
        self.reset_background.bg_lists.resolve_lengths(&fix, vcx);
        for image in self.image_operands() {
            image.resolve_lengths(&fix, vcx);
        }
        if let Some((row, col)) = self.reset_flex.gap.as_mut() {
            fix(row);
            fix(col);
        }
    }
}

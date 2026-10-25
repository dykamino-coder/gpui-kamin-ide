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
        for l in [
            &mut self.width,
            &mut self.height,
            &mut self.min_width,
            &mut self.min_height,
            &mut self.max_width,
            &mut self.max_height,
            &mut self.flex_basis,
            &mut self.svg_path_length,
        ] {
            fix(l);
        }
        // Кегль — в общей группе наследуемых: правится, только если он в
        // единицах окна или контейнера.
        if let Some(size) = self.font_size {
            let mut folded = Some(size);
            fix(&mut folded);
            if folded != Some(size) {
                self.font_size = folded;
            }
        }
        sides(&mut self.padding);
        sides(&mut self.snap.padding);
        sides(&mut self.snap.margin);
        sides(&mut self.margin);
        // Толщина рамки в единицах окна (`border-bottom: 50vh solid`,
        // `monolithic-overflow-021`): без перевода рамка выходила нулевой.
        sides(&mut self.border_width);
        sides(&mut self.inset);
        self.resolve_radius_lengths(fix);
        for gradient in [self.gradient.as_mut(), self.gradient_current.as_mut()]
            .into_iter()
            .flatten()
        {
            gradient.resolve_lengths(fix, vcx);
        }
        self.bg_lists.resolve_lengths(&fix, vcx);
        for image in self.image_operands() {
            image.resolve_lengths(&fix, vcx);
        }
        if let Some((row, col)) = self.gap.as_mut() {
            fix(row);
            fix(col);
        }
    }
}

//! Проверка без записи: есть ли у стиля длины в единицах окна или контейнера, которые решает
//! `Computed::resolve_viewport`. Стиль коробки общий (P-18): копия нужна, только когда решать есть что.

use crate::style::computed::{Computed, Sides};
use crate::style::values::value::Len;
use crate::values::ValueCx;

impl Computed {
    /// Есть ли что решать `resolve_viewport`: длины в единицах окна или
    /// контейнера. Проверка без записи; «да» для значений, которые она не
    /// разбирает (градиенты, картинки, эллиптические радиусы, отступы линий).
    pub fn has_viewport_lengths(&self, vcx: ValueCx<'_>) -> bool {
        let vp = |l: &Option<Len>| match *l {
            Some(Len::Vw(_) | Len::Vh(_)) => true,
            Some(Len::Calc(i)) => {
                let sum = crate::style::values::value::calc_get(i, vcx);
                sum.vw != 0.0 || sum.vh != 0.0 || sum.ext.iter().any(|v| *v != 0.0)
            }
            _ => false,
        };
        let sides = |s: &Sides| [&s.top, &s.right, &s.bottom, &s.left].into_iter().any(vp);
        [
            &self.reset_box_layout.width,
            &self.reset_box_layout.height,
            &self.reset_box_layout.min_width,
            &self.reset_box_layout.min_height,
            &self.reset_box_layout.max_width,
            &self.reset_box_layout.max_height,
            &self.reset_flex.flex_basis,
            &self.reset_svg_math.svg_path_length,
            &self.inh.font.font_size,
            &self.reset_border.radius.tl,
            &self.reset_border.radius.tr,
            &self.reset_border.radius.br,
            &self.reset_border.radius.bl,
        ]
        .into_iter()
        .any(vp)
            || [
                &self.reset_box_layout.padding,
                &self.reset_box_layout.snap.padding,
                &self.reset_box_layout.snap.margin,
                &self.reset_box_layout.margin,
                &self.reset_border.border_width,
                &self.reset_positioned.inset,
            ]
            .into_iter()
            .any(sides)
            || self
                .reset_flex
                .gap
                .is_some_and(|(row, col)| vp(&row) || vp(&col))
            || self.has_decor_insets()
            || self.reset_border.radius_ell.is_some()
            || self.reset_background.gradient.is_some()
            || self.reset_background.gradient_current.is_some()
            || self.reset_background.bg_image.is_some()
            || self.reset_background.gradient_raw.is_some()
            || self.reset_background.bg_lists != Default::default()
            || self.reset_generated.content.is_some()
            || self.reset_mask.mask_image.is_some()
            || self.reset_border.border_image.is_some()
    }
}

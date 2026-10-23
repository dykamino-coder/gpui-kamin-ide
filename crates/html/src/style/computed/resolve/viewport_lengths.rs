//! Проверка без записи: есть ли у стиля длины в единицах окна или контейнера, которые решает
//! `Computed::resolve_viewport`. Стиль коробки общий (P-18): копия нужна, только когда решать есть что.

use crate::document::state::ValueCx;
use crate::style::computed::{Computed, Sides};
use crate::style::values::value::Len;

impl Computed {
    /// Есть ли что решать `resolve_viewport`: длины в единицах окна или
    /// контейнера. Проверка без записи; «да» для значений, которые она не
    /// разбирает (градиенты, картинки, эллиптические радиусы, отступы линий).
    pub(crate) fn has_viewport_lengths(&self, vcx: ValueCx<'_>) -> bool {
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
            &self.width,
            &self.height,
            &self.min_width,
            &self.min_height,
            &self.max_width,
            &self.max_height,
            &self.flex_basis,
            &self.svg_path_length,
            &self.font_size,
            &self.radius.tl,
            &self.radius.tr,
            &self.radius.br,
            &self.radius.bl,
        ]
        .into_iter()
        .any(vp)
            || [
                &self.padding,
                &self.snap.padding,
                &self.snap.margin,
                &self.margin,
                &self.border_width,
                &self.inset,
            ]
            .into_iter()
            .any(sides)
            || self.gap.is_some_and(|(row, col)| vp(&row) || vp(&col))
            || self.has_decor_insets()
            || self.radius_ell.is_some()
            || self.gradient.is_some()
            || self.gradient_current.is_some()
            || self.bg_image.is_some()
            || self.gradient_raw.is_some()
            || self.bg_lists != Default::default()
            || self.content.is_some()
            || self.mask_image.is_some()
            || self.border_image.is_some()
    }
}

//! Геометрические решения оформления используют собственный used-фон коробки.

use super::BoxStyle;
use crate::style::computed::BgClip;
use crate::values::ValueCx;

impl BoxStyle {
    pub fn color_clip(&self, _vcx: ValueCx<'_>) -> Option<BgClip> {
        self.background_paint
            .paints()
            .then(|| self.paint_background().color_clip)
            .flatten()
    }

    pub fn radius_masked(&self, vcx: ValueCx<'_>) -> bool {
        self.style_rc().radius_masked_with(
            super::super::single_corner::box_eligible(self),
            self.paint_shadows().is_empty(),
            vcx,
        )
    }
}

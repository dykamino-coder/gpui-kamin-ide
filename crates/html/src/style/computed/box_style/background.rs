//! Геометрические решения оформления используют собственный used-фон коробки.

use super::BoxStyle;
use crate::document::state::ValueCx;
use crate::style::computed::BgClip;

impl BoxStyle {
    pub(crate) fn color_clip(&self, vcx: ValueCx<'_>) -> Option<BgClip> {
        self.background_paint
            .paints()
            .then(|| self.style_rc().color_clip(vcx))
            .flatten()
    }

    pub(crate) fn radius_masked(&self, vcx: ValueCx<'_>) -> bool {
        !super::super::single_corner::eligible_background(self, self.background_paint)
            && self.style_rc().radius_masked(vcx)
    }
}

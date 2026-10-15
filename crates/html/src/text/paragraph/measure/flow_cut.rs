//! Вырез строки обтекаемыми флоатами (`flow_cut`) и базовая линия струта
//! (`root_base`); вынесено из `padding.rs` ради предела в 250 строк.

use crate::text::paragraph::*;

impl Paragraph {
    pub(crate) fn flow_cut(&self, line_no: usize) -> (f32, f32) {
        if self.flow.0.is_empty() && self.flow.1.is_empty() {
            return (0.0, 0.0);
        }
        let lh = f32::from(self.line_height);
        let (y0, y1) = (line_no as f32 * lh, (line_no as f32 + 1.0) * lh);
        let l = self
            .flow
            .0
            .iter()
            .map(|f| f.cut(y0, y1))
            .fold(0.0f32, f32::max);
        let r = self
            .flow
            .1
            .iter()
            .map(|f| f.cut(y0, y1))
            .fold(0.0f32, f32::max);
        (l, r)
    }
}

impl Paragraph {
    /// Базовая линия струта от верха строки: по замеренной коробке струта
    /// (`strut_box`), иначе по `strut`; без замера — `fallback`.
    pub(super) fn root_base(&self, fallback: f32) -> f32 {
        let lh = f32::from(self.line_height);
        let (sa, sd) = self.strut_box;
        if sa + sd > 0.0 {
            return (lh - (sa + sd)) / 2.0 + sa;
        }
        if self.strut.0 + self.strut.1 > 0.0 {
            return self.strut_base();
        }
        fallback
    }
}

//! Результат оформления строчной коробки для текстовых прогонов, отдельно от CSS.

use crate::style::values::value::Color;

/// Краска полосы разрезанного inline и её физические отступы.
/// Распространяется по дереву коробок; это не наследование CSS background/border/border-radius.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct InlineDecoration {
    pub band_color: Option<Color>,
    pub border_stroke: Option<(Color, [f32; 4])>,
    /// Физические стороны: верх, право, низ, лево.
    pub physical_padding: Option<[f32; 4]>,
    pub corner_radius: Option<f32>,
}

impl InlineDecoration {
    /// Вложенный inline сохраняет полосу предка до появления своей краски.
    pub(crate) fn under(self, parent: Self) -> Self {
        Self {
            band_color: self.band_color.or(parent.band_color),
            border_stroke: self.border_stroke.or(parent.border_stroke),
            physical_padding: self.physical_padding.or(parent.physical_padding),
            corner_radius: self.corner_radius.or(parent.corner_radius),
        }
    }
}

#[cfg(test)]
mod tests;

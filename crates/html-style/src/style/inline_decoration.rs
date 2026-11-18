//! Результат оформления строчной коробки для текстовых прогонов, отдельно от CSS.

use crate::style::values::value::Color;

/// Краска полосы разрезанного inline и её физические отступы.
/// Распространяется по дереву коробок; это не наследование CSS background/border/border-radius.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InlineDecoration {
    pub band_color: Option<Color>,
    pub border_stroke: Option<(Color, [f32; 4])>,
    /// Физические стороны: верх, право, низ, лево.
    pub physical_padding: Option<[f32; 4]>,
    pub corner_radius: Option<f32>,
    /// The node number of the inline box whose band `band_color` is (0: none
    /// or no single box, as for a first-line or first-letter band): the
    /// owner of the band's pieces in a scene layer. 32 bits, as the scene's
    /// run owner (`gpui::TextRun::background_owner`); a number past that is 0.
    pub band_owner: u32,
}

impl InlineDecoration {
    /// Вложенный inline сохраняет полосу предка до появления своей краски.
    pub fn under(self, parent: Self) -> Self {
        Self {
            band_color: self.band_color.or(parent.band_color),
            border_stroke: self.border_stroke.or(parent.border_stroke),
            physical_padding: self.physical_padding.or(parent.physical_padding),
            corner_radius: self.corner_radius.or(parent.corner_radius),
            band_owner: if self.band_color.is_some() {
                self.band_owner
            } else {
                parent.band_owner
            },
        }
    }
}

#[cfg(test)]
mod tests;

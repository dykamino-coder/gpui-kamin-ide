//! `Filter`: выделение одной разобранной цветовой функции по имени в нейтральный
//! фильтр — так растровый путь применяет операции в порядке записи
//! (Filter Effects 1 §5), а не в фиксированном порядке полей.
use super::Filter;
impl Filter {
    /// Последовательность цветовых функций в порядке CSS; сводное значение
    /// используется для программно построенного одиночного фильтра.
    pub(crate) fn apply_operations(
        &self,
        operations: &[Self],
        color: crate::style::values::value::Color,
    ) -> crate::style::values::value::Color {
        if operations.is_empty() {
            self.apply(color)
        } else {
            operations
                .iter()
                .fold(color, |color, operation| operation.apply(color))
        }
    }

    pub(crate) fn color_operation(&self, name: &str) -> Option<Self> {
        let mut one = Self::neutral();
        match name {
            "grayscale" => one.grayscale = self.grayscale,
            "brightness" => one.brightness = self.brightness,
            "saturate" => one.saturate = self.saturate,
            "invert" => one.invert = self.invert,
            "sepia" => one.sepia = self.sepia,
            "opacity" => one.opacity = self.opacity,
            "hue-rotate" => one.hue_rotate = self.hue_rotate,
            "contrast" => one.contrast = self.contrast,
            _ => return None,
        }
        Some(one)
    }
}

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

    /// Покомпонентная интерполяция двух фильтров.
    pub(crate) fn lerp(&self, to: &Self, k: f32) -> Self {
        let lerp = |a: f32, b: f32| a + (b - a) * k;
        Self {
            grayscale: lerp(self.grayscale, to.grayscale),
            brightness: lerp(self.brightness, to.brightness),
            saturate: lerp(self.saturate, to.saturate),
            invert: lerp(self.invert, to.invert),
            sepia: lerp(self.sepia, to.sepia),
            opacity: lerp(self.opacity, to.opacity),
            hue_rotate: lerp(self.hue_rotate, to.hue_rotate),
            contrast: lerp(self.contrast, to.contrast),
            blur: lerp(self.blur, to.blur),
        }
    }

    /// Filter Effects 1 §10 (interpolation of `filter`): функции списков
    /// попарно, недостающие (в том числе у `none`) — начальными значениями
    /// функций другого списка. Списки с разными функциями на одном месте
    /// дают пустую последовательность: остаётся сводное значение.
    pub(crate) fn interpolate_operations(from: &[Self], to: &[Self], k: f32) -> Vec<Self> {
        let neutral = Self::neutral();
        let kinds = |f: &Self| {
            [
                f.grayscale != neutral.grayscale,
                f.brightness != neutral.brightness,
                f.saturate != neutral.saturate,
                f.invert != neutral.invert,
                f.sepia != neutral.sepia,
                f.opacity != neutral.opacity,
                f.hue_rotate != neutral.hue_rotate,
                f.contrast != neutral.contrast,
            ]
        };
        let mut out = Vec::with_capacity(from.len().max(to.len()));
        for i in 0..from.len().max(to.len()) {
            let a = from.get(i).unwrap_or(&neutral);
            let b = to.get(i).unwrap_or(&neutral);
            let (ka, kb) = (kinds(a), kinds(b));
            if ka.contains(&true) && kb.contains(&true) && ka != kb {
                return Vec::new();
            }
            out.push(a.lerp(b, k));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolated_list_keeps_authored_order_against_none() {
        let mut brightness = Filter::neutral();
        brightness.brightness = 0.0;
        let mut invert = Filter::neutral();
        invert.invert = 1.0;
        let half = Filter::interpolate_operations(&[brightness, invert], &[], 0.5);
        assert_eq!(half.len(), 2);
        assert_eq!((half[0].brightness, half[0].invert), (0.5, 0.0));
        assert_eq!((half[1].brightness, half[1].invert), (1.0, 0.5));
        let back = Filter::interpolate_operations(&[], &[brightness, invert], 0.25);
        assert_eq!((back[0].brightness, back[1].invert), (0.75, 0.25));
    }

    #[test]
    fn mismatched_functions_fall_back_to_the_folded_value() {
        let mut sepia = Filter::neutral();
        sepia.sepia = 1.0;
        let mut invert = Filter::neutral();
        invert.invert = 1.0;
        assert!(Filter::interpolate_operations(&[sepia], &[invert], 0.5).is_empty());
    }
}

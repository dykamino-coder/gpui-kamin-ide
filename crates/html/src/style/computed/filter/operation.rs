//! Extract one parsed color operation, retaining CSS Filter Effects 1 §5 order.
use super::Filter;
impl Filter {
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

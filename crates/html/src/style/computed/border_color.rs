//! Reset all border color longhands when a single-value shorthand wins the cascade.

use super::Computed;
use crate::style::values::value::Color;

impl Computed {
    pub(super) fn apply_single_border_color(&mut self, value: &str) {
        let current = value.eq_ignore_ascii_case("currentcolor");
        let color = if current { None } else { Color::parse(value) };
        if !current && color.is_none() {
            return;
        }
        // CSS Backgrounds 3 §4.1: the shorthand sets all four longhands,
        // including sides set by an earlier border or side shorthand.
        self.border_color_is_current = current;
        self.border_color = color;
        self.border_colors = [color; 4];
        self.border_side_current = [current; 4];
    }
}

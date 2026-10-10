//! Validate the color-only image() syntax while preserving currentColor.

use crate::style::values::value::Color;

pub(crate) fn parse(raw: &str) -> Option<&str> {
    let color = raw.trim().strip_prefix("image(")?.strip_suffix(')')?.trim();
    (color.eq_ignore_ascii_case("currentcolor") || Color::parse(color).is_some()).then_some(color)
}

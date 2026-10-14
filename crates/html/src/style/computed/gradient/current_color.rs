//! Resolve gradient stop colors in the element's used currentColor context.

use super::{Color, Computed, Gradient, split_outside_parens};
use crate::style::values::color_space::resolve_relative;
use std::borrow::Cow;

const BLACK: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};

pub(crate) fn parse_gradient(raw: &str) -> Option<Gradient> {
    parse_gradient_with_current(raw, BLACK)
}

pub(crate) fn parse_gradient_with_current(raw: &str, current: Color) -> Option<Gradient> {
    super::parse::parse_gradient(&resolve_gradient_colors(raw, current))
}

/// Keep the authored expression on the style: inherited currentColor uses
/// the descendant's color (CSS Color 4 §6.4), including relative colors.
impl Computed {
    pub(crate) fn resolve_gradient_current_color(&mut self) {
        if (self.gradient.is_some()
            || self.background_all_inherit
            || self.inherit_bits & super::inh::BG_IMAGE != 0)
            && let Some(raw) = self
                .gradient_raw
                .as_deref()
                .or(self.gradient_current.as_deref())
            && raw.contains("currentcolor")
        {
            self.gradient = parse_gradient_with_current(raw, self.color.unwrap_or(BLACK));
        }
    }
}

/// Raster cache keys must distinguish the same expression on two elements.
/// Only stop colors are substituted; directions and positions stay authored.
pub(crate) fn resolve_gradient_colors(raw: &str, current: Color) -> Cow<'_, str> {
    if !(raw.contains("currentcolor") || raw.contains("from "))
        || !raw
            .split('(')
            .next()
            .is_some_and(|name| name.ends_with("gradient"))
    {
        return Cow::Borrowed(raw);
    }
    let (Some(open), Some(close)) = (raw.find('('), raw.rfind(')')) else {
        return Cow::Borrowed(raw);
    };
    let parts: Vec<String> = crate::style::css::split_args(&raw[open + 1..close])
        .into_iter()
        .map(|part| {
            let mut words = split_outside_parens(part);
            if let Some(token) = words.first_mut()
                && (token.contains("currentcolor") || token.contains("from "))
                && let Some(color) = resolve_relative(token, current)
            {
                // Relative RGB is a modern color, so preserve the default
                // OKLab interpolation rather than serializing it as rgba().
                *token = if token == "currentcolor" {
                    format!(
                        "rgba({},{},{},{})",
                        color.r * 255.0,
                        color.g * 255.0,
                        color.b * 255.0,
                        color.a
                    )
                } else {
                    format!(
                        "color(srgb {} {} {} / {})",
                        color.r, color.g, color.b, color.a
                    )
                };
            }
            words.join(" ")
        })
        .collect();
    Cow::Owned(format!("{}{})", &raw[..=open], parts.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::GradSpace;

    #[test]
    fn shorthand_inheritance_keeps_the_native_background_route() {
        let mut parent = Computed::default();
        parent.apply_one(
            "background",
            "linear-gradient(to right, currentcolor, limegreen)",
        );
        let mut child = Computed::default();
        child.apply_one("background", "inherit");
        child.apply_one("color", "limegreen");
        let child = crate::style::cascade::inherit::inherit(&parent, &child);
        assert!(child.gradient_raw.is_none());
        let gradient = child.gradient.as_ref().unwrap();
        assert_eq!(gradient.from, Color::parse("limegreen").unwrap());
        assert_eq!(gradient.to, gradient.from);
        // Canvas propagation moves the painted gradient away from body,
        // while descendants still inherit its authored image expression.
        parent.gradient = None;
        let child = crate::style::cascade::inherit::inherit(&parent, &child);
        assert_eq!(
            child.gradient.unwrap().from,
            Color::parse("limegreen").unwrap()
        );
    }

    #[test]
    fn relative_stops_use_the_inherited_element_color() {
        let mut own = Computed::default();
        own.apply_one(
            "background-image",
            "linear-gradient(rgb(from currentcolor r g b), rgb(from currentcolor r g b))",
        );
        let mut parent = Computed::default();
        parent.apply_one("color", "blue");
        let blue = crate::style::cascade::inherit::inherit(&parent, &own);
        let gradient = blue.gradient.as_ref().unwrap();
        assert_eq!(gradient.from, Color::parse("blue").unwrap());
        assert!(matches!(gradient.space, GradSpace::Oklab));
        let mut child = Computed::default();
        child.apply_one("color", "green");
        child.apply_one("background-image", "inherit");
        let green = crate::style::cascade::inherit::inherit(&blue, &child);
        assert_eq!(green.gradient.unwrap().from, Color::parse("green").unwrap());
        assert!(green.gradient_raw.unwrap().contains("currentcolor"));
    }

    #[test]
    fn cancelled_current_color_image_is_not_reinherited() {
        for replacement in ["none", "image(red)"] {
            let mut parent = Computed::default();
            parent.apply_one(
                "background-image",
                "linear-gradient(currentcolor, currentcolor)",
            );
            parent.apply_one("background-image", replacement);
            let mut child = Computed::default();
            child.apply_one("background-image", "inherit");
            let child = crate::style::cascade::inherit::inherit(&parent, &child);
            assert!(parent.gradient_current.is_none());
            assert!(child.gradient.is_none());
        }
    }
}

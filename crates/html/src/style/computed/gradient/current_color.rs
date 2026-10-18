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
    super::parse::parse_gradient(raw, current)
}

/// Keep the authored expression on the style: inherited currentColor uses
/// the descendant's color (CSS Color 4 §6.4), including relative colors.
impl Computed {
    pub(crate) fn resolve_gradient_current_color(&mut self) {
        if (self.gradient.is_some()
            || self.background_all_inherit
            || self.inherit_bits & super::inh::BG_IMAGE != 0)
            && let Some(template) = self.gradient_current.as_ref()
        {
            let mut gradient = self.gradient.clone().unwrap_or_else(|| template.clone());
            gradient.resolve_colors(self.color.unwrap_or(BLACK));
            self.gradient = Some(gradient);
        }
    }
}

impl Gradient {
    pub(crate) fn resolve_colors(&mut self, current: Color) {
        if self.colors.is_empty() {
            return;
        }
        for (index, expression) in self.colors.iter().enumerate() {
            let color = expression.resolve_for_interpolation(current);
            if let Some(stop) = self.stops.get_mut(index) {
                stop.0 = color;
            }
            if let Some(stop) = self.stops_px.get_mut(index) {
                stop.0 = color;
            }
            if let Some(stop) = self.stops_raw.get_mut(index) {
                stop.0 = color;
            }
        }
        if let Some(first) = self.stops.first() {
            self.from = first.0;
        }
        if let Some(last) = self.stops.last() {
            self.to = last.0;
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
    #[test]
    fn typed_stop_colors_preserve_double_positions_without_css() {
        let mut parent = Computed::default();
        parent.apply_one(
            "background-image",
            "linear-gradient(in srgb, currentcolor 20px 30px, blue 100px)",
        );
        // The inherited operand must not consult a textual paint source.
        parent.gradient_raw = Some("not a gradient".into());
        let mut child = Computed::default();
        child.apply_one("background-image", "inherit");
        child.apply_one("color", "red");
        let child = crate::style::cascade::inherit::inherit(&parent, &child);
        let g = child.gradient.unwrap();
        let red = Color::parse("red").unwrap();
        assert_eq!(
            g.stops_px,
            vec![
                (red, 20.0),
                (red, 30.0),
                (Color::parse("blue").unwrap(), 100.0)
            ]
        );
        assert_eq!(
            g.stops_raw.iter().map(|s| s.2).collect::<Vec<_>>(),
            vec![Some(20.0), Some(30.0), Some(100.0)]
        );
    }

    #[test]
    fn inherited_template_preserves_computed_font_positions() {
        let mut own = Computed::default();
        own.apply_one("font-size", "20px");
        own.apply_one(
            "background-image",
            "linear-gradient(currentcolor 1em, blue 2em)",
        );
        let parent = crate::style::cascade::inherit::inherit(&Computed::default(), &own);
        let mut child = Computed::default();
        child.apply_one("font-size", "80px");
        child.apply_one("background-image", "inherit");
        child.apply_one("color", "green");
        let child = crate::style::cascade::inherit::inherit(&parent, &child);
        let g = child.gradient.unwrap();
        assert_eq!(g.stops_px[0], (Color::parse("green").unwrap(), 20.0));
        assert_eq!(g.stops_px[1].1, 40.0);
        assert!(child.gradient_em.is_none());
    }
    #[test]
    fn radial_first_stop_accepts_current_and_relative_colors() {
        let red = Color::parse("red").unwrap();
        for token in ["currentcolor", "rgb(from currentcolor r g b)"] {
            let g = parse_gradient_with_current(&format!("radial-gradient({token}, blue)"), red)
                .unwrap();
            assert_eq!(g.stops.len(), 2);
            assert_eq!(g.from, red);
            assert_eq!(g.to, Color::parse("blue").unwrap());
        }
    }
}

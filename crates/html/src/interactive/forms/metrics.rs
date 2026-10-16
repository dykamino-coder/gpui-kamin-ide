//! Native control metrics, independent of the embedding application's UI.

use crate::style::computed::Computed;
use crate::style::values::value::Len;

// Blink html.css / LayoutThemeDefault and WebThemeEngine on Windows.
const SIZES: [(&str, [f32; 2]); 4] = [
    ("checkbox", [13., 13.]),
    ("radio", [13., 13.]),
    ("range", [129., 16.]),
    ("color", [50., 27.]),
];

pub(super) fn size(kind: &str) -> [f32; 2] {
    SIZES.iter().find(|(name, _)| *name == kind).unwrap().1
}

/// HTML rendering: text entry widgets cannot use a line-height below normal,
/// including when appearance is none. Resolve normal on both paths, so it
/// cannot accidentally inherit a shorter GPUI line-height from a wrapper.
pub(super) fn text_style(style: &Computed) -> Computed {
    let mut c = style.clone();
    let font = match c.font_size {
        Some(Len::Px(v)) => v,
        _ => 16.,
    };
    let family = c.font_family.as_deref().unwrap_or("");
    let normal = crate::text::metrics::normal_px(crate::text::metrics::normal_line(family), font);
    let line = c
        .line_height
        .and_then(|l| crate::text::metrics::fallback_len_px(l, family, font))
        .unwrap_or(normal);
    c.line_height = Some(Len::Px(line.max(normal)));
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_metrics_use_browser_defaults() {
        assert_eq!(size("checkbox"), [13., 13.]);
        assert_eq!(size("radio"), size("checkbox"));
        assert_eq!(size("range"), [129., 16.]);
        assert_eq!(size("color"), [50., 27.]);
    }

    #[test]
    fn text_entry_clamps_only_short_lines() {
        let mut c = Computed {
            font_size: Some(Len::Px(60.)),
            ..Computed::default()
        };
        let normal = text_style(&c).line_height;
        c.line_height = Some(Len::Px(1.));
        assert_eq!(text_style(&c).line_height, normal);
        c.line_height = Some(Len::Px(200.));
        assert_eq!(text_style(&c).line_height, c.line_height);
    }
}

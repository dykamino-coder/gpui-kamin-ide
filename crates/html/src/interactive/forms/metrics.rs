//! Native control metrics, independent of the embedding application's UI.

use crate::style::computed::Computed;
use crate::style::computed::FontCx;
use crate::style::values::value::Len;

// Blink html.css / LayoutThemeDefault and WebThemeEngine on Windows.
pub(super) enum ControlKind {
    Checkbox,
    Radio,
    Range,
    Color,
}

pub(super) fn size(kind: ControlKind) -> [f32; 2] {
    match kind {
        ControlKind::Checkbox | ControlKind::Radio => [13., 13.],
        ControlKind::Range => [129., 16.],
        ControlKind::Color => [50., 27.],
    }
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
        .and_then(|l| l.to_px(None, FontCx::new(family, font)))
        .unwrap_or(normal);
    c.line_height = Some(Len::Px(line.max(normal)));
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_metrics_use_browser_defaults() {
        assert_eq!(size(ControlKind::Checkbox), [13., 13.]);
        assert_eq!(size(ControlKind::Radio), size(ControlKind::Checkbox));
        assert_eq!(size(ControlKind::Range), [129., 16.]);
        assert_eq!(size(ControlKind::Color), [50., 27.]);
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

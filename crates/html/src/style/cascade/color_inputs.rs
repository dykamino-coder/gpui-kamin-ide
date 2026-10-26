//! Общий расчёт цвета для подготовки документа и оформления коробки.

use crate::style::computed::{Computed, flat_background::BackgroundImageValues};
use crate::style::values::value::Color;

pub(super) struct TextClipColors {
    pub color: Option<Color>,
    pub raw: Option<Color>,
    pub fill: Option<Color>,
}

pub(super) fn text_clip_colors(
    parent: &Computed,
    declared: &Computed,
    own: &Computed,
    c: &Computed,
    element: bool,
    images: &dyn BackgroundImageValues,
) -> TextClipColors {
    let mut out = TextClipColors {
        color: c.color,
        raw: c.text_clip_raw,
        fill: c.text_clip_fill,
    };
    let black = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    let out_of_flow = matches!(
        own.position,
        Some(crate::style::computed::Position::Absolute)
            | Some(crate::style::computed::Position::Fixed)
    );
    if let Some(fill) = crate::style::computed::flat_background::text_clip_fill(c, images) {
        let foreground = c.text_clip_raw.or(c.color);
        out.raw = foreground;
        out.fill = Some(fill);
        out.color = Some(crate::style::values::composite::over(
            foreground.unwrap_or(black),
            fill,
        ));
    } else if parent.text_clip_fill.is_some() && out_of_flow {
        out.fill = None;
        out.raw = None;
        if !element && !declared.declares_color() {
            out.color = parent.text_clip_raw;
        }
    } else if let Some(fill) = parent.text_clip_fill {
        out.fill = Some(fill);
        if element || declared.declares_color() {
            out.raw = c.color;
            out.color = Some(crate::style::values::composite::over(
                c.color.unwrap_or(black),
                fill,
            ));
        } else {
            out.raw = parent.text_clip_raw;
        }
    }
    out
}

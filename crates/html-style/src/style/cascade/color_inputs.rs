//! Общий расчёт цвета для подготовки документа и оформления коробки.

use crate::style::computed::flat_background::BackgroundImageValues;
use crate::style::values::value::Color;

use crate::style::decoration_paint::ForegroundPaint;

pub(super) struct TextClipInput {
    pub parent: ForegroundPaint,
    pub foreground: ForegroundPaint,
    pub element: bool,
    pub declares_color: bool,
    pub out_of_flow: bool,
    pub background: crate::style::background_paint::BackgroundPaint,
}

pub(super) fn text_clip_colors(
    context: TextClipInput,
    background_values: &crate::style::background_paint::BackgroundValues,
    images: &dyn BackgroundImageValues,
) -> ForegroundPaint {
    let TextClipInput {
        parent,
        foreground: input,
        element,
        declares_color,
        out_of_flow,
        background,
    } = context;
    let mut out = input;
    let black = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    if let Some(fill) = background
        .paints()
        .then(|| crate::style::computed::flat_background::text_clip_fill(background_values, images))
        .flatten()
    {
        let foreground = input.raw.or(input.color);
        out.raw = foreground;
        out.fill = Some(fill);
        out.color = Some(crate::style::values::composite::over(
            foreground.unwrap_or(black),
            fill,
        ));
    } else if parent.fill.is_some() && out_of_flow {
        out.fill = None;
        out.raw = None;
        if !element && !declares_color {
            out.color = parent.raw;
        }
    } else if let Some(fill) = parent.fill {
        out.fill = Some(fill);
        if element || declares_color {
            out.raw = input.color;
            out.color = Some(crate::style::values::composite::over(
                input.color.unwrap_or(black),
                fill,
            ));
        } else {
            out.raw = parent.raw;
        }
    }
    out
}

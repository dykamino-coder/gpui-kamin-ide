//! Build emphasis spans with their line geometry and clipped background fill.

use super::Piece;
use crate::style::values::value::Len;

/// Куски со знаком акцента. Знак набирается в половину кегля своей базы
/// (css-text-decor-3 §5.3, как аннотация руби с `font-size: 50%`) и встаёт
/// на край строчной коробки куска (`line-height` куска, `normal` — доля
/// `normal` шрифта абзаца).
pub fn emphasis_spans(
    pieces: &[Piece],
    base_size: f32,
    normal: f32,
) -> Vec<crate::text::paragraph::EmphSpan> {
    let mut out = Vec::new();
    let mut at = 0usize;
    for p in pieces {
        let Piece::Text { text, style } = p else {
            continue;
        };
        let end = at + text.len();
        if let Some(mark) = style.text_emphasis.as_deref().filter(|m| !m.is_empty())
            && text.chars().any(|c| !c.is_whitespace() && c != '\u{feff}')
        {
            let size = match style.font_size {
                Some(Len::Px(v)) => v,
                _ => base_size,
            };
            let line_height = match style.line_height {
                Some(Len::Px(v)) => v,
                Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size,
                _ => normal * size,
            };
            out.push(crate::text::paragraph::EmphSpan {
                range: at..end,
                under: style.emphasis_under,
                size: size * 0.5,
                line_height,
                mark: mark.to_string(),
                // CSS Backgrounds 4 background-clip:text includes emphasis
                // geometry regardless of the mark's foreground transparency.
                // CurrentColor already carries the composited text fill;
                // explicit emphasis colors need the same composition here.
                color: style.emphasis_color.map(|color| {
                    style
                        .text_clip_fill
                        .map_or(color, |fill| crate::paint::background::over(color, fill))
                        .to_hsla()
                }),
            });
        }
        at = end;
    }
    out
}

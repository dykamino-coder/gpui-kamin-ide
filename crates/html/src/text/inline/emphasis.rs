//! Знаки акцента `text-emphasis` по кускам абзаца (css-text-decor-3 §5.3–5.4): из `Piece::Text`
//! собираются отрезки `EmphSpan` с байтовым диапазоном, размером знака (половина кегля базы),
//! интерлиньяжем куска, стороной (над/под, в вертикальном письме — право/лево, `auto` по языку),
//! формой по режиму письма и цветом с учётом `background-clip: text`; руби получает отметку о
//! знаке для раскладки слоёв.

use super::Piece;
use crate::style::values::value::Color;

/// Куски со знаком акцента. Знак набирается в половину кегля своей базы
/// (css-text-decor-3 §5.3, как аннотация руби с `font-size: 50%`) и встаёт
/// на край строчной коробки куска (`line-height` куска, `normal` — доля
/// `normal` шрифта абзаца).
pub(crate) fn emphasis_spans(
    pieces: &[Piece],
    base_size: f32,
    normal: f32,
    rotated: Option<bool>,
    // Слои аннотаций руби-базы, в которой абзац собирается (`RubyContext::layers`).
    ruby: Option<&std::rc::Rc<crate::text::ruby::layers::Layers>>,
    vcx: crate::document::state::ValueCx<'_>,
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
            let size = style.used_font(base_size, vcx).size_px;
            let line_height = style
                .numeric_line_px(base_size)
                .unwrap_or_else(|| crate::text::metrics::normal_px(normal, size, vcx));
            out.push(crate::text::paragraph::EmphSpan {
                ruby: ruby.cloned(),
                range: at..end,
                // Повёрнутый абзац вертикального письма (`rotated` — поворот
                // против часовой, `sideways-lr`): «над» строкой после
                // поворота по часовой — ПРАВО. В вертикальном письме сторону
                // знака задают `right`/`left` (css-text-decor-3 §5.4), а
                // `over`/`under` — только в горизонтальном.
                under: match rotated {
                    Some(ccw) => style.emphasis_left != ccw,
                    None if style.emphasis_position_auto == Some(true) => {
                        style.lang.as_deref().is_some_and(|lang| {
                            lang.split('-')
                                .next()
                                .is_some_and(|lang| lang.eq_ignore_ascii_case("zh"))
                        })
                    }
                    None => style.emphasis_under,
                },
                size: size * 0.5,
                line_height,
                // CSS Text Decoration 4: an omitted shape follows the used
                // typographic mode, including inherited emphasis on a child.
                mark: if style.emphasis_auto_shape
                    && rotated.is_some()
                    && !style.wm().sideways()
                    && !style.wm().text_sideways()
                {
                    if mark == "\u{25CB}" {
                        "\u{FE46}"
                    } else {
                        "\u{FE45}"
                    }
                    .to_string()
                } else {
                    mark.to_string()
                },
                // CSS Backgrounds 4 background-clip:text includes emphasis
                // geometry regardless of the mark's foreground transparency.
                // CurrentColor already carries the composited text fill;
                // explicit emphasis colors need the same composition here.
                color: style.text_shadow_ink.map(Color::to_hsla).or_else(|| {
                    let color = style.emphasis_color.as_ref().filter(|c| !c.is_current());
                    color.map(|color| {
                        let color = style.used_color(color);
                        style
                            .text_clip_fill
                            .map_or(color, |fill| {
                                crate::style::values::composite::over(color, fill)
                            })
                            .to_hsla()
                    })
                }),
            });
        }
        if let Some(span) = out.last().filter(|span| span.range.end == end)
            && let Some(ruby) = &span.ruby
        {
            ruby.mark(span.under, span.size);
        }
        at = end;
    }
    out
}

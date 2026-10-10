//! First line for inline; split out to keep the owning module within 250 lines.

use super::Piece;
use super::first_line_background;
use crate::style::computed::Computed;

/// Одеть первые `at` байт абзаца в стиль первой строки (`::first-line`).
///
/// Длину первой строки считает замер: до переноса она неизвестна.
/// `block` — стиль самого блока: значения, которые кусок не унаследовал от
/// него, а получил от своего строчного элемента, первая строка не трогает.
pub fn style_first_line(
    pieces: Vec<Piece>,
    at: usize,
    style: &Computed,
    block: &Computed,
) -> Vec<Piece> {
    let mut out: Vec<Piece> = Vec::with_capacity(pieces.len() + 1);
    let mut seen = 0usize;
    for p in pieces {
        match p {
            Piece::Text { text, style: own } if seen < at => {
                let dress = |base: &Computed| {
                    let mut c = base.clone();
                    c.font_size = style.font_size.or(base.font_size);
                    // The fictional `::first-line` tag sequence wraps the
                    // line's inline elements (css-pseudo-4
                    // §first-line-inheritance): an element's own color wins
                    // (`display-contents-first-line-002`: green spans).
                    if base.color == block.color {
                        c.color = style.color.or(base.color);
                    }
                    c.font_weight = style.font_weight.or(base.font_weight);
                    c.italic = style.italic.or(base.italic);
                    // Возможности шрифта первой строки, в том числе запрет
                    // подмены начертания (`nsyw`/`nsys`, css-fonts-4 §6.5):
                    // без них полужирный и курсив `::first-line` синтезировались
                    // вопреки `font-synthesis-*: none`. Дописываются ПОСЛЕ
                    // своих: при повторе тега побеждает первая строка.
                    c.font_features.extend(style.font_features.iter().cloned());
                    c.font_kerning = style.font_kerning.or(base.font_kerning);
                    c.font_alternates = style
                        .font_alternates
                        .clone()
                        .or_else(|| base.font_alternates.clone());
                    crate::style::computed::font_family::inherit(&mut c, style, base);
                    c.font_settings = style
                        .font_settings
                        .clone()
                        .or_else(|| base.font_settings.clone());
                    // Коробочная часть первой строки: интерлиньяж и подложка
                    // (css-pseudo-4 §4.1; first-line-line-height-001/002).
                    c.background = style.background.or(base.background);
                    // Красит подложку прогон текста, и берёт он её из
                    // `inline_bg`: слой первой строки накладывается уже ПОСЛЕ
                    // сборки кусков, когда `inline_bg` посчитан по своему
                    // стилю (`c25-pseudo-elmnt-000`: зелёной полосы не было).
                    c.inline_bg =
                        first_line_background::paint_color(base.inline_bg, style.background);
                    c.line_height = style.line_height.or(base.line_height);
                    // ПРОБОВАЛИ И ОТКАТИЛИ: переносить сюда и сдвиг по
                    // вертикали (§5.12.1 относит `vertical-align` к свойствам
                    // `::first-line`). Проба по 372 парам семей `first-line-*`
                    // и `first-letter-*`: `first-line-pseudo-012` 4.66 -> 4.94,
                    // флипов ноль. Краска поднимается, а коробка нет: высоту
                    // абзаца заявляет `float::FirstLine` (`measure_first_line`),
                    // и подъёма она не знает. Возвращаться вместе с ней.
                    c
                };
                let len = text.len();
                if seen + len <= at {
                    let dressed = dress(&own);
                    out.push(Piece::Text {
                        text,
                        style: dressed,
                    });
                } else {
                    let mut cut = at - seen;
                    while cut < text.len() && !text.is_char_boundary(cut) {
                        cut += 1;
                    }
                    out.push(Piece::Text {
                        text: text[..cut].to_string(),
                        style: dress(&own),
                    });
                    if cut < text.len() {
                        out.push(Piece::Text {
                            text: text[cut..].to_string(),
                            style: own,
                        });
                    }
                }
                seen += len;
            }
            other => {
                if let Piece::Text { text, .. } = &other {
                    seen += text.len();
                }
                out.push(other);
            }
        }
    }
    out
}

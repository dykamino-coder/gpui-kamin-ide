//! `::first-line` поверх собранных кусков абзаца: первые `at` байт (длину первой строки даёт
//! замер) одеваются в стиль псевдоэлемента — шрифт, возможности, цвет (свой цвет элемента
//! побеждает, css-pseudo-4 §first-line-inheritance), интерлиньяж и подложка; знаки
//! управления направлением не считаются и не делятся.

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
            Piece::Text { style: ref own, .. } if own.inline_control => out.push(p),
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
                    c.font_synth = (
                        style.font_synth.0.or(base.font_synth.0),
                        style.font_synth.1.or(base.font_synth.1),
                        style.font_synth.2.or(base.font_synth.2),
                    );
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
                    // Не переносить сюда сдвиг по вертикали (§5.12.1 относит
                    // `vertical-align` к свойствам `::first-line`): краска
                    // поднимается, а коробка нет — высоту абзаца заявляет
                    // `float::FirstLine` (`measure_first_line`), и подъёма она
                    // не знает (`first-line-pseudo-012`). Возвращать вместе с ней.
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

#[cfg(test)]
mod tests {
    //! Source offsets must survive generated bidi isolation around inline content.

    use super::*;

    #[test]
    fn generated_controls_do_not_shorten_the_styled_source_range() {
        let piece = |text: &str, inline_control| Piece::Text {
            text: text.into(),
            style: Computed {
                inline_control,
                ..Computed::default()
            },
        };
        let first = {
            let mut with_inherited = Computed {
                ..Computed::default()
            };
            with_inherited.font_weight = Some(700);
            with_inherited
        };
        for (generated, at, expected) in [(true, 2, "ab"), (false, 5, "ab")] {
            let pieces = style_first_line(
                vec![piece("\u{2068}", generated), piece("abc", false)],
                at,
                &first,
                &Computed::default(),
            );
            let styled: String = pieces
                .iter()
                .filter_map(|p| match p {
                    Piece::Text { text, style }
                        if !text.contains('\u{2068}') && style.font_weight == Some(700) =>
                    {
                        Some(text.as_str())
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(styled, expected);
        }
    }
}

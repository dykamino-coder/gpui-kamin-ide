//! Инлайн-поток: текст с вкраплениями `<b>`, `<a>`, `<code>` внутри абзаца.
//!
//! Здесь лежит главное расхождение GPUI с HTML. В браузере абзац — это поток
//! строк, куда встраиваются куски любого размера, и строка растёт под самый
//! высокий из них. В GPUI текстовый блок — лист раскладки: размер шрифта у него
//! ОДИН на весь блок (`shape_text` принимает `font_size` скаляром), и метрики
//! строки тоже одни.
//!
//! Отсюда две стратегии, и выбор между ними делается по факту содержимого:
//!
//! * **Один блок текста.** Если все куски абзаца одного размера и ни один не
//!   несёт собственного бокса (фон, рамка, отступы), они собираются в один
//!   `StyledText` с прогонами. Перенос строк тогда честный — как в браузере,
//!   по словам, сквозь границы `<b>` и `<a>`.
//! * **Строка из элементов.** Иначе куски становятся отдельными элементами в
//!   гибкой строке с переносом. Перенос идёт по кускам, а не по словам внутри
//!   них — это заметно на длинном `<code>`, но зато размеры и боксы честные.
//!
//! Первая ветка покрывает подавляющее большинство: жирный, курсив, ссылка,
//! цвет. Вторая включается там, где без неё пришлось бы врать про размер.

use crate::computed::Computed;
use crate::dom::{Element, Node};
use crate::value::{Color, Len};
use gpui::{
    AnyElement, FontStyle, FontWeight, HighlightStyle, IntoElement, ParentElement, SharedString,
    Styled, StyledText, TextRun, TextStyle, UnderlineStyle,
};

/// Кусок инлайн-содержимого: либо текст со своим стилем, либо готовый элемент
/// (картинка, кнопка — то, что текстом не является).
pub enum Piece {
    Text { text: String, style: Computed },
    Atom(AnyElement),
}

/// Собрать инлайн-куски из детей узла.
pub fn collect(
    children: &[Node],
    inherited: &Computed,
    atom: &mut dyn FnMut(&Element) -> Option<AnyElement>,
) -> Vec<Piece> {
    let mut out = vec![];
    for child in children {
        match child {
            Node::Text(t) => {
                let text = normalize_spaces(t);
                if !text.is_empty() {
                    out.push(Piece::Text {
                        text,
                        style: inherited.clone(),
                    });
                }
            }
            Node::Element(e) => {
                if e.tag == "br" {
                    out.push(Piece::Text {
                        text: "\n".into(),
                        style: inherited.clone(),
                    });
                    continue;
                }
                if let Some(el) = atom(e) {
                    out.push(Piece::Atom(el));
                    continue;
                }
                let merged = inherit(inherited, &e.style);
                out.extend(collect(&e.children, &merged, atom));
            }
        }
    }
    out
}

/// Наследование: в CSS вниз идут только текстовые свойства. Бокс-свойства
/// (отступы, фон) принадлежат самому элементу и вниз не передаются.
pub fn inherit(parent: &Computed, own: &Computed) -> Computed {
    let mut c = own.clone();
    c.color = own.color.or(parent.color);
    c.font_size = own.font_size.or(parent.font_size);
    c.font_weight = own.font_weight.or(parent.font_weight);
    c.italic = own.italic.or(parent.italic);
    c.underline = own.underline.or(parent.underline);
    c.line_through = own.line_through.or(parent.line_through);
    c.line_height = own.line_height.or(parent.line_height);
    c.text_align = own.text_align.or(parent.text_align);
    c.monospace = own.monospace.or(parent.monospace);
    c.nowrap = own.nowrap.or(parent.nowrap);
    c
}

/// Схлопывание пробелов, как в HTML: переводы строк и повторы — один пробел.
fn normalize_spaces(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut prev_space = false;
    for ch in raw.chars() {
        let is_space = ch.is_whitespace();
        if is_space {
            if !prev_space {
                out.push(' ');
            }
        } else {
            out.push(ch);
        }
        prev_space = is_space;
    }
    out
}

/// Можно ли собрать всё в один текстовый блок: одинаковый размер шрифта и ни
/// одного не-текстового куска.
pub fn single_block(pieces: &[Piece], base_size: f32) -> bool {
    pieces.iter().all(|p| match p {
        Piece::Atom(_) => false,
        Piece::Text { style, .. } => match style.font_size {
            Some(Len::Px(v)) => (v - base_size).abs() < 0.01,
            None => true,
            _ => false,
        },
    })
}

/// Один `StyledText` с прогонами — честный перенос по словам сквозь границы
/// `<b>`/`<a>`/`<span>`.
pub fn as_styled_text(pieces: &[Piece], base: &TextStyle) -> Option<StyledText> {
    let mut text = String::new();
    let mut runs: Vec<TextRun> = vec![];
    for p in pieces {
        let Piece::Text { text: t, style } = p else {
            return None;
        };
        if t.is_empty() {
            continue;
        }
        text.push_str(t);
        runs.push(run_for(t, style, base));
    }
    if text.is_empty() {
        return None;
    }
    Some(StyledText::new(SharedString::from(text)).with_runs(runs))
}

fn run_for(text: &str, style: &Computed, base: &TextStyle) -> TextRun {
    let mut font = base.font();
    if style.monospace == Some(true) {
        font.family = "JetBrains Mono".into();
    }
    if let Some(w) = style.font_weight {
        font.weight = FontWeight(w as f32);
    }
    if style.italic == Some(true) {
        font.style = FontStyle::Italic;
    }
    let color = style.color.map(Color::to_hsla).unwrap_or(base.color);
    TextRun {
        len: text.len(),
        font,
        color,
        background_color: None,
        underline: style.underline.unwrap_or(false).then(|| UnderlineStyle {
            thickness: gpui::px(1.),
            color: Some(color),
            wavy: false,
        }),
        strikethrough: style
            .line_through
            .unwrap_or(false)
            .then(|| gpui::StrikethroughStyle {
                thickness: gpui::px(1.),
                color: Some(color),
            }),
    }
}

/// Подсветка для куска текста — используется, когда прогоны накладываются на
/// готовый текст (например, при подсветке кода).
pub fn highlight_for(style: &Computed) -> HighlightStyle {
    HighlightStyle {
        color: style.color.map(Color::to_hsla),
        font_weight: style.font_weight.map(|w| FontWeight(w as f32)),
        font_style: style.italic.and_then(|i| i.then_some(FontStyle::Italic)),
        ..Default::default()
    }
}

/// Запасная ветка: гибкая строка из отдельных элементов.
pub fn as_wrapped_row(
    pieces: Vec<Piece>,
    render_text: &mut dyn FnMut(String, &Computed) -> AnyElement,
) -> AnyElement {
    let mut row = gpui::div().flex().flex_wrap().items_baseline();
    for p in pieces {
        row = match p {
            Piece::Atom(el) => row.child(el),
            Piece::Text { text, style } => row.child(render_text(text, &style)),
        };
    }
    row.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css::parse_decls;

    fn styled(css: &str) -> Computed {
        let mut c = Computed::default();
        c.apply_decls(&parse_decls(css));
        c
    }

    #[test]
    fn spaces_collapse_like_html() {
        assert_eq!(normalize_spaces("  два\n\tслова  "), " два слова ");
        assert_eq!(normalize_spaces("a\n\nb"), "a b");
    }

    #[test]
    fn same_size_pieces_go_into_one_block() {
        let pieces = vec![
            Piece::Text {
                text: "обычный ".into(),
                style: styled(""),
            },
            Piece::Text {
                text: "жирный".into(),
                style: styled("font-weight: 700"),
            },
        ];
        assert!(single_block(&pieces, 13.0), "вес не мешает единому блоку");
    }

    #[test]
    fn different_size_forces_the_row_fallback() {
        let pieces = vec![
            Piece::Text {
                text: "обычный ".into(),
                style: styled(""),
            },
            Piece::Text {
                text: "крупный".into(),
                style: styled("font-size: 24px"),
            },
        ];
        assert!(
            !single_block(&pieces, 13.0),
            "иной размер = единым блоком нельзя"
        );
    }

    #[test]
    fn inheritance_carries_text_not_box() {
        let parent = styled("color: #ff0000; padding: 10px; font-size: 20px");
        let child = styled("font-weight: 700");
        let merged = inherit(&parent, &child);
        assert_eq!(merged.color.map(|c| c.r), Some(1.0), "цвет наследуется");
        assert_eq!(merged.font_size, Some(Len::Px(20.0)), "размер наследуется");
        assert_eq!(merged.font_weight, Some(700), "свой вес сохранён");
        assert_eq!(merged.padding.top, None, "отступ родителя вниз не идёт");
    }

    #[test]
    fn own_style_wins_over_inherited() {
        let parent = styled("color: #ff0000");
        let child = styled("color: #0000ff");
        assert_eq!(inherit(&parent, &child).color.map(|c| c.b), Some(1.0));
    }
}

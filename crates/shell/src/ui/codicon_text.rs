//! `renderCodiconText` (`renderer/utils/codicon-text.tsx`): строка вида
//! `«$(check) Готово»` рисуется как ГЛИФ кодикона + текст. Без разбора
//! `$(...)` печаталось буквально — так было в Quick Pick (ревью ц.35).

use gpui::prelude::*;
use gpui::{Div, div, px};

/// Куски строки: текст и имена иконок между `$(` и `)`.
pub fn split(text: &str) -> Vec<Piece<'_>> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("$(") {
        let after = &rest[start + 2..];
        let Some(end) = after.find(')') else { break };
        if start > 0 {
            out.push(Piece::Text(&rest[..start]));
        }
        out.push(Piece::Icon(&after[..end]));
        rest = &after[end + 1..];
    }
    if !rest.is_empty() {
        out.push(Piece::Text(rest));
    }
    out
}

pub enum Piece<'a> {
    Text(&'a str),
    /// Имя кодикона без обёртки `$( )`.
    Icon(&'a str),
}

/// Ряд «глиф + текст» тем же кеглем, что у строки. Неизвестное имя иконки
/// печатается как есть — в оригинале пустой `<span class="codicon codicon-…">`
/// тоже ничего не рисует, но текст терять нельзя.
pub fn render(text: &str, icon_size: f32) -> Div {
    let mut row = div().flex().items_center().gap(px(4.0));
    for piece in split(text) {
        row = match piece {
            Piece::Text(t) => row.child(t.to_string()),
            Piece::Icon(name) => match crate::ui::codicon_map::codicon_by_name(name) {
                Some(glyph) => row.child(crate::ui::icon::codicon(glyph, icon_size)),
                None => row.child(format!("$({name})")),
            },
        };
    }
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(text: &str) -> String {
        split(text)
            .into_iter()
            .map(|p| match p {
                Piece::Text(t) => format!("T[{t}]"),
                Piece::Icon(i) => format!("I[{i}]"),
            })
            .collect()
    }

    #[test]
    fn splits_like_the_original_regex() {
        assert_eq!(shape("plain"), "T[plain]");
        assert_eq!(shape("$(check)"), "I[check]");
        assert_eq!(shape("a $(check) b"), "T[a ]I[check]T[ b]");
        assert_eq!(shape("$(a)$(b)"), "I[a]I[b]");
        // Незакрытая скобка — это просто текст
        assert_eq!(shape("$(oops"), "T[$(oops]");
    }
}

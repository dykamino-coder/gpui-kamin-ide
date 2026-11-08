//! Текстовые узлы при обходе: какие пробельные куски доживают до дерева.
//!
//! Пустым по CSS считается узел из одних схлопываемых знаков (пробел, таб,
//! CR, LF; идеографический U+3000 — не пустой). Под `white-space: pre*`
//! ничего не выбрасывается. Пробельный узел между двумя строчными соседями
//! становится одним пробелом (css-text-3 §4.1.1), а не отбивкой разметки.
//! Соседние текстовые узлы склеиваются, чтобы схлопывание не давало двух
//! пробелов вокруг комментария.

use crate::dom::*;
use markup5ever_rcdom::{Handle, NodeData};

pub(super) fn walk_text(
    text: Text,
    preserve: bool,
    level: &[Handle],
    level_pos: usize,
    out: &mut Vec<Node>,
) {
    // Пустой узел ПО CSS — только схлопываемые пробелы
    // (`space`/`tab`/`CR`/`LF`). `str::trim` снимает весь юникодный
    // пробел, и узел из идеографических U+3000 отбрасывался прямо на
    // разборе: строка из них не доезжала до раскладки вовсе
    // (`trailing-ideographic-space-017`).
    let collapsible = |c: char| matches!(c, ' ' | '\t' | '\r' | '\n');
    // Под `white-space: pre*` схлопывания нет вовсе: узел из одного
    // перевода строки — это жёсткий разрыв, и выбрасывать его нельзя
    // (`word-space-transform-011`: `あ<wbr>い<wbr>\n<wbr>う` шло одной
    // строкой, потому что перевод пропадал ещё на разборе).
    // Пробельный узел без пробела (`</span>\n\t<span>`) между двумя
    // строчными соседями — это тоже схлопываемый пробел строки
    // (css-text-3 §4.1.1: перевод строки и табуляция превращаются в
    // пробел), а не отбивка разметки между блоками. Выброшенный, он
    // склеивал соседние слова: `multicol-basic-001…004` — «XXXX» двух
    // span сливались в одно слово, колонки резались не там.
    let between_inline = !preserve
        && text.chars().all(collapsible)
        && matches!(out.last(), Some(Node::Element(prev))
            if prev.inline && prev.style.cascaded().reset_box_layout.display.is_none() && prev.tag != "br")
        && level[level_pos + 1..]
            .iter()
            .find(|h| !matches!(h.data, NodeData::Comment { .. }))
            .is_some_and(|h| match &h.data {
                NodeData::Text { contents } => !contents.borrow().chars().all(collapsible),
                NodeData::Element { name, .. } => {
                    let tag = local_name(&name.local);
                    tag != "br" && INLINE_TAGS.contains(&tag.as_str())
                }
                _ => false,
            });
    let text = if between_inline {
        Text::from_source(" ".to_string(), text.source())
    } else {
        text
    };
    if preserve || !text.chars().all(collapsible) || text.contains(' ') {
        // Комментарий разрезает пробельный кусок надвое, а схлопывание
        // работает по одному узлу — выходило два пробела подряд.
        // Соседние текстовые узлы склеиваются в один отрезок.
        match out.last_mut() {
            Some(Node::Text(prev)) if !preserve => prev.append(&text),
            _ => out.push(Node::Text(text)),
        }
    }
}

/// Upper estimate of the children `walk` keeps for `handle` (0 when the box is
/// hidden): elements and text nodes that are not pure markup line breaks or
/// tabs (`walk_text` drops those outside `white-space: pre*`). Used as the box
/// children capacity: counting every DOM child over-reserves for the newline
/// nodes between block siblings (one per element in pretty-printed markup).
pub(super) fn kept_children_hint(handle: &Handle, hidden: bool) -> usize {
    if hidden {
        return 0;
    }
    handle
        .children
        .borrow()
        .iter()
        .filter(|h| match &h.data {
            NodeData::Element { .. } => true,
            NodeData::Text { contents } => contents
                .borrow()
                .chars()
                .any(|c| !matches!(c, '\t' | '\r' | '\n')),
            _ => false,
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::kept_children_hint;
    use html5ever::tendril::TendrilSink;

    #[test]
    fn children_hint_skips_markup_newlines_and_comments_but_keeps_spaces() {
        let dom =
            html5ever::parse_document(markup5ever_rcdom::RcDom::default(), Default::default())
                .one("<body>\n<p>a</p>\n<!-- c -->\n<p>b</p> <i>x</i>\n\t</body>");
        let html = dom.document.children.borrow()[0].clone();
        let body = html.children.borrow()[1].clone();
        // p, p, " " (a space between inline siblings survives), i
        assert_eq!(kept_children_hint(&body, false), 4);
        assert_eq!(kept_children_hint(&body, true), 0);
    }
}

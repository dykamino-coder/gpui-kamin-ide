//! Text nodes for walk; split out to keep the owning module within 250 lines.

use crate::dom::*;
use markup5ever_rcdom::{Handle, NodeData};

pub(super) fn walk_text(
    text: String,
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
            if prev.inline && prev.style.display.is_none() && prev.tag != "br")
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
        " ".to_string()
    } else {
        text
    };
    if preserve || !text.chars().all(collapsible) || text.contains(' ') {
        // Комментарий разрезает пробельный кусок надвое, а схлопывание
        // работает по одному узлу — выходило два пробела подряд.
        // Соседние текстовые узлы склеиваются в один отрезок.
        match out.last_mut() {
            Some(Node::Text(prev)) if !preserve => prev.push_str(&text),
            _ => out.push(Node::Text(text)),
        }
    }
}

//! Инертное поддерево: обычный поток текста и блоков без собственных слоёв.
//!
//! Позиционированные потомки уходят в слой содержащего блока, флоаты — в
//! полосы контейнера, а трансформация, прозрачность и смешение открывают
//! контекст наложения с отложенной краской. Замещаемые элементы, формы и
//! ссылки держат ресурсы, состояние или ввод. Такое поддерево не пропускается.

use crate::dom::{Element, Node};
use crate::style::computed::Position;

/// Теги обычного потока, чья коробка — только блок или строка текста.
const TAGS: &[&str] = &[
    "div",
    "p",
    "span",
    "b",
    "i",
    "em",
    "strong",
    "u",
    "s",
    "small",
    "code",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "section",
    "article",
    "header",
    "footer",
    "main",
    "nav",
    "aside",
    "blockquote",
];

/// Таблица раскладывается внутри своей коробки (CSS 2.1 §17.4): её ячейки —
/// обычный поток, а сетка — только геометрия.
const TABLE: &[&str] = &[
    "table", "caption", "colgroup", "col", "thead", "tbody", "tfoot", "tr", "td", "th",
];

/// Каждый элемент поддерева `e` инертен (см. модуль).
pub(super) fn plain(e: &Element) -> bool {
    all_elements(e, |e| {
        let style = e.style.used();
        let tag = e.tag.as_str();
        let own = (TAGS.contains(&tag) || TABLE.contains(&tag))
            && e.hover.is_none()
            && e.anim.is_none()
            && e.first_letter.is_none()
            && e.first_line.is_none()
            && e.list_item.is_none()
            && matches!(
                style.reset_positioned.position,
                None | Some(Position::Static)
            )
            && style.reset_positioned.z_index.is_none()
            && style.reset_box_layout.float.is_none_or(|f| f == 0)
            && style.reset_transforms.transform.is_none()
            && style.reset_transforms.translate.is_none()
            && style.reset_effects.opacity.is_none_or(|o| o >= 1.0)
            && style.reset_effects.blend.is_none_or(|b| b == 0);
        own
    })
}

/// В поддереве `e` есть флоат.
pub(super) fn floats(e: &Element) -> bool {
    !all_elements(e, |e| {
        e.style.used().reset_box_layout.float.is_none_or(|f| f == 0)
    })
}

/// Every element of the subtree of `e`, in document order, passes `f`; the
/// walk stops at the first that does not. The stack holds one child iterator
/// per open level, so a block of thousands of inline children costs a few
/// words per level of depth instead of a list of every pending sibling.
pub(super) fn all_elements(e: &Element, mut f: impl FnMut(&Element) -> bool) -> bool {
    if !f(e) {
        return false;
    }
    let mut open = vec![e.children.iter()];
    while let Some(level) = open.last_mut() {
        match level.next() {
            Some(Node::Element(child)) => {
                if !f(child) {
                    return false;
                }
                open.push(child.children.iter());
            }
            Some(_) => {}
            None => {
                open.pop();
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::{Marks, Text};
    use crate::style::computed::Computed;

    fn el(tag: &str, children: Vec<Node>) -> Element {
        Element::synthetic(tag, Computed::default(), Marks::default(), children)
    }

    #[test]
    fn text_blocks_are_plain_and_replaced_or_positioned_descendants_are_not() {
        let text = Node::Text(Text::from_source("a".into(), None));
        assert!(plain(&el(
            "p",
            vec![text.clone(), Node::Element(el("b", vec![]))]
        )));
        assert!(!plain(&el("div", vec![Node::Element(el("img", vec![]))])));
        let mut moved = el("span", vec![text]);
        moved.style.used_mut().reset_positioned.position = Some(Position::Absolute);
        assert!(!plain(&el("div", vec![Node::Element(moved.clone())])));
        assert!(!floats(&el("div", vec![Node::Element(moved)])));
        let mut floated = el("span", vec![]);
        floated.style.used_mut().reset_box_layout.float = Some(1);
        assert!(floats(&el("div", vec![Node::Element(floated)])));
    }
}

/// Обёртка `html`/`body` (или её отсутствие) не читает базовых линий и
/// строк крайних детей потока: блочная коробка без псевдоэлементов первой
/// строки и буквы, без обрезки текстовых краёв (css-inline-3 §5) и без
/// обрезки содержимого, под которой повтор краски был бы под другой маской.
pub(super) fn edges_free(e: Option<&Element>) -> bool {
    use crate::style::computed::{Display, Overflow};
    let Some(e) = e else { return true };
    let style = e.style.used();
    let b = &style.reset_box_layout;
    let visible = |o: Option<Overflow>| matches!(o, None | Some(Overflow::Visible));
    e.first_line.is_none()
        && e.first_letter.is_none()
        && matches!(b.display, None | Some(Display::Block))
        && visible(b.overflow_x)
        && visible(b.overflow_y)
        && !style.reset_text_box.text_box_trim_start
        && !style.reset_text_box.text_box_trim_end
}

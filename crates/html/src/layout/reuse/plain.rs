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

/// Вердикт `plain` одного блочного ребёнка на время его сборки: решение о
/// записи повтора (`FlowCx::mark`) и запись пропуска (`FlowCx::record`)
/// спрашивают его оба, а обход поддерева выполняется один раз.
#[derive(Default)]
pub(super) struct Inert(std::cell::OnceCell<bool>);

impl Inert {
    /// `plain(e)` для ребёнка, ради которого создан вердикт.
    pub(super) fn of(&self, e: &Element) -> bool {
        *self.0.get_or_init(|| plain(e))
    }
}

/// Каждый элемент поддерева `e` инертен (см. модуль).
pub(super) fn plain(e: &Element) -> bool {
    all_elements(e, inert)
}

/// Элемент `e` сам инертен: обычный поток или таблица, без собственного слоя.
fn inert(e: &Element) -> bool {
    let style = e.style.used();
    let tag = e.tag.as_str();
    (TAGS.contains(&tag) || TABLE.contains(&tag))
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
        && style.reset_effects.blend.is_none_or(|b| b == 0)
}

/// Поддерево `e` можно повторить на экране (`replay`): инертно как `plain`,
/// без единиц области просмотра в стилях и без вертикального письма, гибких
/// и сеточных коробок: их сборка читает размеры окна напрямую, а не через
/// ограничения, чьи чтения записываются.
pub(super) fn replayable(e: &Element, vcx: crate::values::ValueCx<'_>) -> bool {
    all_elements(e, |e| replayable_node(e, vcx))
}

/// Элемент `e` сам по себе не мешает повтору поддерева (`replayable`).
pub(super) fn replayable_node(e: &Element, vcx: crate::values::ValueCx<'_>) -> bool {
    use crate::style::computed::Display;
    let style = e.style.used();
    inert(e)
        && !style.wm().vertical()
        && !style.has_viewport_lengths(vcx)
        && !matches!(
            style.reset_box_layout.display,
            Some(Display::Flex | Display::InlineFlex | Display::Grid | Display::InlineGrid)
        )
}

/// A table element (`far::replayable` admits none).
pub(super) fn table(e: &Element) -> bool {
    TABLE.contains(&e.tag.as_str())
}

/// Блок `e` заполняет доступную ширину (CSS 2.1 §10.3.3): `width: auto` без
/// `max-width`, не таблица (её авто-ширина — css-tables-3 §3.9.4). Его
/// ширина меняется с каждой шириной потока, повтор там не встанет на место.
pub(super) fn fills_width(e: &Element) -> bool {
    use crate::style::computed::Display;
    let layout = &e.style.used().reset_box_layout;
    let table = e.tag.as_str() == "table"
        || matches!(layout.display, Some(Display::Table | Display::InlineTable));
    !table
        && layout
            .width
            .as_ref()
            .is_none_or(|w| matches!(w, crate::style::values::value::Len::Auto))
        && layout.max_width.is_none()
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
/// words per level of depth instead of a list of every pending sibling. The
/// children of `e` itself are walked in place and a childless element opens
/// no level: a block of text and empty elements allocates nothing.
pub(super) fn all_elements(e: &Element, mut f: impl FnMut(&Element) -> bool) -> bool {
    if !f(e) {
        return false;
    }
    let mut top = e.children.iter();
    let mut open: Vec<std::slice::Iter<'_, Node>> = Vec::new();
    loop {
        let next = match open.last_mut() {
            Some(level) => level.next(),
            None => top.next(),
        };
        match next {
            Some(Node::Element(child)) => {
                if !f(child) {
                    return false;
                }
                if !child.children.is_empty() {
                    open.push(child.children.iter());
                }
            }
            Some(_) => {}
            None => {
                if open.pop().is_none() {
                    return true;
                }
            }
        }
    }
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

    #[test]
    fn inert_verdict_is_computed_once_per_child_build() {
        let inert = Inert::default();
        assert!(inert.of(&el("p", vec![])));
        // The second question is answered from the first walk, not a new one.
        assert!(inert.of(&el("img", vec![])));
        assert!(!Inert::default().of(&el("img", vec![])));
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

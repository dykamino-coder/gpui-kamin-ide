//! Анонимные табличные боксы и табличные роли в стилизованном дереве документа.

use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::style::computed::{Computed, Display};
use std::borrow::Cow;

mod wrapping;
pub(crate) use wrapping::InlineTables;

/// Табличная роль бесхозного узла: `Some(true)` — структурная (ряд, группа
/// рядов, колонка, группа колонок), `Some(false)` — ячейка, `None` — обычная
/// коробка.
///
/// Колонка приходит с `Display::None` и живой меткой роли: коробки она не
/// даёт, но прогон рвать не должна и обязана попасть в ту же анонимную
/// таблицу.
///
/// Плавающее и абсолютное по §9.7 блокифицируются и табличной ролью быть
/// перестают. Блокификации у нас пока нет, поэтому такие узлы проход не
/// трогает — их судьбу решает прежний путь.
fn anon_role(n: &Node) -> Option<bool> {
    let Node::Element(e) = n else { return None };
    if e.style
        .used()
        .reset_box_layout
        .float
        .is_some_and(|f| f != 0)
        || matches!(
            e.style.used().reset_positioned.position,
            Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
        )
    {
        return None;
    }
    if col_role(e).is_some() {
        return Some(true);
    }
    if is_cell(e) {
        return Some(false);
    }
    match e.style.used().reset_box_layout.display {
        Some(Display::TableRow) | Some(Display::TableRowGroup) => Some(true),
        _ => match e.tag.as_str() {
            "tr" | "thead" | "tbody" | "tfoot" => Some(true),
            _ => None,
        },
    }
}

/// §17.2.1, шаг 3: ПОСЛЕДОВАТЕЛЬНЫЕ братья с табличной ролью, стоящие в
/// не-табличном родителе, заворачиваются в ОДНУ анонимную таблицу.
///
/// Общая таблица сохраняет последовательные ряды в одной табличной сетке.
/// Починку содержимого (ряд вокруг ячеек, ячейка вокруг прочего) делает
/// `fixup_table_children` внутри собранной таблицы.
/// Without table-role siblings the list is returned as is, not copied.
pub fn wrap_anon_tables(nodes: &[Node]) -> Cow<'_, [Node]> {
    match wrapping::tables(nodes, Display::Table, &Default::default())
        .expect("anonymous table allocation")
    {
        Some(nodes) => Cow::Owned(nodes.into_vec()),
        None => Cow::Borrowed(nodes),
    }
}

/// Общий проход полного документа и локальной публикации коробок.
pub fn inline_anon_tables(
    admission: &crate::budget::admission::Admission,
    nodes: Vec<Node>,
) -> Result<Vec<Node>, crate::budget::admission::Error> {
    super::normalization::apply(&InlineTables, nodes, false, admission)
}

pub fn anon_element(tag: &str, children: Vec<Node>) -> Element {
    // CSS 2.1 §17.2.1: an anonymous cell is not an HTML `td`; the UA rule
    // `td { vertical-align: inherit }` (HTML §15.3.9) does not reach it.
    Element {
        marks: crate::dom::Marks {
            anonymous_cell: tag == "td",
            ..Default::default()
        },
        list_item: None,
        node_id: 0,
        anim: None,
        tag: tag.into(),
        style: Computed::default().into(),
        hover: None,
        first_letter: None,
        first_line: None,
        children: children.into(),
        attrs: Vec::new(),
        inline: false,
    }
}

/// Колоночная роль элемента: тег ИЛИ `display` (§17.2.1). `Some(false)` —
/// колонка, `Some(true)` — группа колонок.
pub fn col_role(el: &Element) -> Option<bool> {
    match el.tag.as_str() {
        "col" => Some(false),
        "colgroup" => Some(true),
        _ => match el.style.used().reset_context.col_role {
            Some(0) => Some(false),
            Some(1) => Some(true),
            _ => None,
        },
    }
}

/// Подпись стоит у block-end края коробки таблицы: собственное computed
/// значение `caption-side` подписи (CSS 2.2 §17.4.1). Свойство наследуемое:
/// подпись, ребёнок таблицы, получает его от таблицы и её предков каскадом.
pub fn caption_at_bottom(caption: &Element) -> bool {
    caption.style.used().inh.table_list.caption_bottom == Some(true)
}

pub fn is_cell(element: &Element) -> bool {
    match element.style.used().reset_box_layout.display {
        Some(Display::TableCell) => true,
        None => matches!(element.tag.as_str(), "td" | "th"),
        _ => false,
    }
}

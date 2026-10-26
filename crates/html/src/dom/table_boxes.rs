//! Анонимные табличные боксы и табличные роли в стилизованном дереве документа.

use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::style::computed::{Computed, Display};

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
    if e.style.used().float.is_some_and(|f| f != 0)
        || matches!(
            e.style.used().position,
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
    match e.style.used().display {
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
pub(crate) fn wrap_anon_tables(nodes: &[Node]) -> Vec<Node> {
    wrap_anon_tables_as(nodes, Display::Table)
}

/// CSS 2.1 §17.2.1 step 3 for INLINE parents: "If the box's parent is an
/// inline box, then an anonymous inline-table box must be generated" around
/// each run of consecutive proper table child boxes. Without it every orphan
/// cell in a line got its own wrapper table, and the white space between
/// consecutive cells (removed by §17.2.1 step 1) stayed in the line.
pub(crate) fn inline_anon_tables(nodes: &mut [Node]) {
    for n in nodes.iter_mut() {
        let Node::Element(e) = n else { continue };
        inline_anon_tables(&mut e.children);
        if e.style.used().display.is_none()
            && e.inline
            && e.children.iter().any(|c| anon_role(c).is_some())
        {
            e.children = wrap_anon_tables_as(&e.children, Display::InlineTable).into();
        }
    }
}

fn wrap_anon_tables_as(nodes: &[Node], display: Display) -> Vec<Node> {
    if !nodes.iter().any(|n| anon_role(n).is_some()) {
        return nodes.to_vec();
    }
    let flush = |run: &mut Vec<Node>, out: &mut Vec<Node>| {
        if run.is_empty() {
            return;
        }
        let mut t = anon_element("table", std::mem::take(run));
        t.style.cascaded_mut().display = Some(display);
        t.marks.anonymous_wrapper = true;
        // Свой номер узла: по нему таблица просит буферы проб. Нулевой у
        // всех анонимных узлов общий, и две таблицы делили бы один буфер —
        // первая забрала бы его, вторая осталась пустой.
        t.node_id = match t.children.first() {
            Some(Node::Element(e)) => e.node_id.rotate_left(1) ^ 0x7ab1_e000,
            _ => 0,
        };
        out.push(Node::Element(t));
    };
    let mut out: Vec<Node> = vec![];
    let mut run: Vec<Node> = vec![];
    let mut gap: Vec<Node> = vec![];
    for n in nodes {
        if anon_role(n).is_some() {
            run.append(&mut gap);
            run.push(n.clone());
        } else if is_blank(n) && !run.is_empty() {
            // Пробел между табличными братьями прогона не рвёт (§17.2.1,
            // «consecutive»). Место ему решит следующий узел: внутри прогона
            // он уйдёт в таблицу и там пропадёт, за прогоном — останется
            // снаружи.
            gap.push(n.clone());
        } else {
            flush(&mut run, &mut out);
            out.append(&mut gap);
            out.push(n.clone());
        }
    }
    flush(&mut run, &mut out);
    out.append(&mut gap);
    out
}

pub(crate) fn anon_element(tag: &str, children: Vec<Node>) -> Element {
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
pub(crate) fn col_role(el: &Element) -> Option<bool> {
    match el.tag.as_str() {
        "col" => Some(false),
        "colgroup" => Some(true),
        _ => match el.style.used().col_role {
            Some(0) => Some(false),
            Some(1) => Some(true),
            _ => None,
        },
    }
}

pub(crate) fn is_cell(element: &Element) -> bool {
    match element.style.used().display {
        Some(Display::TableCell) => true,
        None => matches!(element.tag.as_str(), "td" | "th"),
        _ => false,
    }
}

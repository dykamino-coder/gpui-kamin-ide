//! Достройка детей таблицы и ряда анонимными объектами (CSS 2.1 §17.2.1).

use super::anon_element;
use crate::dom::{Element, Node};
use crate::layout::table::columns::col_role;
use crate::layout::table::{is_cell, table_roles};
use crate::style::cascade::inherit::inherit;
use crate::style::computed::Display;

/// Починка детей таблицы (css-tables-3 §3): `display: contents` растворить,
/// бесхозные ячейки и непустой текст завернуть в анонимный ряд.
/// Чинит СОДЕРЖИМОЕ ряда (css-tables-3 §fixup): `display: contents`
/// растворяется с наследованием, последовательные не-ячейки сливаются в
/// одну анонимную ячейку, а вложенный ряд выталкивается ОТДЕЛЬНЫМ рядом
/// после текущего.
pub(crate) fn fixup_row_children(row: &Element) -> Vec<Node> {
    fn walk(nodes: &[Node], cells: &mut Vec<Node>, run: &mut Vec<Node>) {
        for child in nodes {
            match child {
                Node::Element(el) if el.style.display == Some(Display::Contents) => {
                    // Дети растворённого получают его наследуемое (цвет,
                    // шрифт) — слитый стиль передаётся вниз донором.
                    let merged: Vec<Node> = el
                        .children
                        .iter()
                        .cloned()
                        .map(|n| match n {
                            Node::Element(mut ge) => {
                                ge.style = inherit(&el.style, &ge.style);
                                Node::Element(ge)
                            }
                            // Голый текст стиля не несёт: наследуемое от
                            // растворённого доносит строчная обёртка.
                            Node::Text(t) if !t.trim().is_empty() => {
                                let mut span = anon_element("span", vec![Node::Text(t)]);
                                span.style = el.style.clone();
                                // Сам растворённый display не переносится —
                                // иначе обёртка растворилась бы следом.
                                span.style.display = None;
                                span.inline = true;
                                Node::Element(span)
                            }
                            other => other,
                        })
                        .collect();
                    walk(&merged, cells, run);
                }
                // §17.2.1 шаг 2: ребёнок ряда, который не ячейка, уходит в
                // АНОНИМНУЮ ЯЧЕЙКУ этого же ряда — ряд внутри ряда тоже.
                // Прежде он выталкивался сестринским рядом, и таблица
                // получала лишнюю строку (`table-anonymous-objects-090`).
                Node::Element(el)
                    if el.tag == "tr"
                        || matches!(
                            el.style.display,
                            Some(Display::TableRow) | Some(Display::TableRowGroup)
                        ) =>
                {
                    run.push(child.clone());
                    let _ = el;
                }
                Node::Element(el) if is_cell(el) => {
                    table_roles::flush_inline(cells, run);
                    cells.push(child.clone());
                }
                // Whitespace is classified after collecting the anonymous
                // inline box, not before its non-whitespace content is seen.
                Node::Text(_) => run.push(child.clone()),
                Node::Element(_) => run.push(child.clone()),
            }
        }
    }
    let needs_fix = row.children.iter().any(|c| match c {
        Node::Element(el) => {
            el.style.display == Some(Display::Contents)
                || el.tag == "tr"
                || matches!(
                    el.style.display,
                    Some(Display::TableRow) | Some(Display::TableRowGroup)
                )
                || !is_cell(el)
        }
        Node::Text(t) => !t.trim().is_empty(),
    });
    if !needs_fix {
        return vec![Node::Element(row.clone())];
    }
    let (mut cells, mut run) = (vec![], vec![]);
    walk(&row.children, &mut cells, &mut run);
    table_roles::flush_inline(&mut cells, &mut run);
    let mut fixed = row.clone();
    fixed.children = cells;
    vec![Node::Element(fixed)]
}

pub(crate) fn fixup_table_children(children: &[Node]) -> Vec<Node> {
    let mut out: Vec<Node> = vec![];
    let mut stray: Vec<Node> = vec![];
    fn flush(stray: &mut Vec<Node>, out: &mut Vec<Node>) {
        if stray.is_empty() {
            return;
        }
        // ПОСЛЕДОВАТЕЛЬНЫЕ не-ячейки сливаются в ОДНУ анонимную ячейку
        // (css-tables-3 §consecutive-boxes): два inline-block с текстом между
        // ними — одна ячейка с общей строкой, а не ячейка на каждого.
        let mut cells: Vec<Node> = vec![];
        let mut run: Vec<Node> = vec![];
        for n in std::mem::take(stray) {
            match n {
                Node::Element(e) if is_cell(&e) => {
                    if !run.is_empty() {
                        cells.push(Node::Element(anon_element("td", std::mem::take(&mut run))));
                    }
                    cells.push(Node::Element(e));
                }
                other => run.push(other),
            }
        }
        if !run.is_empty() {
            cells.push(Node::Element(anon_element("td", run)));
        }
        out.push(Node::Element(anon_element("tr", cells)));
    }
    for child in children {
        match child {
            Node::Element(el) if el.style.display == Some(Display::Contents) => {
                // Дети идут в таблицу со СЛИТЫМ стилем: наследуемое от
                // растворённого элемента (цвет, шрифт) обязано дойти.
                for grand in fixup_table_children(&el.children) {
                    match grand {
                        Node::Element(mut ge) => {
                            ge.style = inherit(&el.style, &ge.style);
                            let row = ge.tag == "tr"
                                || matches!(
                                    ge.style.display,
                                    Some(Display::TableRow) | Some(Display::TableRowGroup)
                                )
                                || matches!(ge.tag.as_str(), "thead" | "tbody" | "tfoot");
                            if row {
                                flush(&mut stray, &mut out);
                                out.push(Node::Element(ge));
                            } else {
                                stray.push(Node::Element(ge));
                            }
                        }
                        text => stray.push(text),
                    }
                }
            }
            Node::Element(el) => {
                // Колоночные элементы — не содержимое: их читают дорожки.
                // Роль задаётся тегом ИЛИ `display` (§17.2.1).
                if col_role(el).is_some() {
                    continue;
                }
                let group = el.style.display == Some(Display::TableRowGroup)
                    || matches!(el.tag.as_str(), "thead" | "tbody" | "tfoot");
                let row = el.tag == "tr"
                    || el.style.display == Some(Display::TableRow)
                    || el.tag == "caption";
                let is_cap = el.tag == "caption" || el.style.is_caption == Some(true);
                if group {
                    // Группа рядов чинится ИЗНУТРИ тоже: contents и бесхозное
                    // содержимое встречаются и там.
                    flush(&mut stray, &mut out);
                    let mut copy = el.clone();
                    copy.children = fixup_table_children(&el.children);
                    out.push(Node::Element(copy));
                } else if is_cap {
                    flush(&mut stray, &mut out);
                    out.push(child.clone());
                } else if row {
                    flush(&mut stray, &mut out);
                    out.extend(fixup_row_children(el));
                } else {
                    stray.push(child.clone());
                }
            }
            Node::Text(t) if !t.trim().is_empty() => stray.push(child.clone()),
            // CSS 2.1 §17.2.1 step 1.3: white space is dropped only when each
            // existing immediate sibling is an internal table box or caption.
            // Next to an inline (`<span>a</span> <span>b</span>`) it belongs
            // to the anonymous cell's line, where it separates the words.
            Node::Text(_) => {
                let ix = children
                    .iter()
                    .position(|n| std::ptr::eq(n, child))
                    .unwrap_or(0);
                let inline_like = |n: Option<&Node>| match n {
                    Some(Node::Text(t)) => !t.trim().is_empty(),
                    Some(Node::Element(el)) => {
                        el.style.display != Some(Display::Contents)
                            && col_role(el).is_none()
                            && !is_cell(el)
                            && el.tag != "caption"
                            && el.style.is_caption != Some(true)
                            && el.tag != "tr"
                            && !matches!(el.tag.as_str(), "thead" | "tbody" | "tfoot")
                            && !matches!(
                                el.style.display,
                                Some(Display::TableRow) | Some(Display::TableRowGroup)
                            )
                    }
                    _ => false,
                };
                let prev = ix.checked_sub(1).and_then(|i| children.get(i));
                if inline_like(prev) || inline_like(children.get(ix + 1)) {
                    stray.push(child.clone());
                }
            }
        }
    }
    flush(&mut stray, &mut out);
    out
}

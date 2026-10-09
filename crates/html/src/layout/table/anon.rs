//! Анонимные объекты таблицы.
// owner: A

use crate::style::cascade::inherit::inherit;
use crate::dom::{Element, Node};
use crate::layout::positioned::relative::relative_shift;
use crate::layout::table::columns::col_role;
use crate::layout::table::{is_cell, table_roles};
use crate::render::is_blank;
use crate::style::computed::{Computed, Display};

/// Сдвиг, фон и СТИЛЬ ГРУППЫ строк: письмо/шрифт с `<tbody>` наследуются в
/// ряды и ячейки, хотя своей коробки у группы нет (ch-units-vrl-006).
/// Сдвиг, фон и САМА ГРУППА рядов: от неё нужны и наследуемый стиль, и
/// `node_id` с рамками — кромки группы строит ряд.
pub(crate) type RowCarry<'a> = (f32, f32, Option<crate::style::values::value::Color>, Option<&'a Element>);

pub(crate) fn collect_rows<'a>(
    nodes: &'a [Node],
    parent: Option<&'a Element>,
    carry: RowCarry<'a>,
    out: &mut Vec<(&'a Element, RowCarry<'a>)>,
) {
    for n in nodes {
        if let Node::Element(e) = n {
            let (dx, dy) = relative_shift(e, parent);
            // Фон группы строк рисуют ЯЧЕЙКИ: своей коробки у группы в общей
            // сетке не остаётся, и заливка пропадала молча
            // (`position-relative-table-tbody-left`: зелёная коробка не
            // рисовалась вовсе, из-под неё светило красное).
            let shift = (
                carry.0 + dx,
                carry.1 + dy,
                e.style.background.or(carry.2),
                carry.3,
            );
            // `visibility: collapse` на ряде или группе рядов ВЫБРАСЫВАЕТ их
            // из сетки, как и на колонке: ряды не рисуются, а их высота из
            // таблицы уходит (css-tables-3 §visibility-collapse). Прежде
            // читалась только колонка, и схлопнутый ряд оставлял пустую
            // полосу.
            if e.style.collapsed == Some(true) {
                continue;
            }
            // Роль задаётся тегом ИЛИ стилем: разметка на `div` с
            // `display: table-row` встречается не реже настоящих таблиц.
            if e.tag == "tr" || e.style.display == Some(Display::TableRow) {
                out.push((e, shift));
            } else if e.tag == "thead"
                || e.tag == "tbody"
                || e.tag == "tfoot"
                || e.style.display == Some(Display::TableRowGroup)
            {
                // Группа с КАРТИНКОЙ красит и цвет САМА — полосой `grp_band`
                // (§14.2: цвет лежит ПОД картинкой). Ячейка его не дублирует:
                // она рисуется после полосы, и цвет прятал бы картинку
                // (`background-repeat-applies-to-001/002/003`). У ряда та же
                // отсечка стоит давно.
                //
                // Первый заход сюда дал ровно ноль (+3 / −3): тройка
                // `background-attachment-applies-to-*` уходила 0.48 -> 1.44,
                // потому что полоса не знала `attachment: fixed`. Теперь
                // знает, и отсечка стала чистым приобретением.
                let picture = e.style.bg_image.is_some() || e.style.gradient_raw.is_some();
                let deeper = (
                    shift.0,
                    shift.1,
                    if picture { None } else { shift.2 },
                    Some(e),
                );
                collect_rows(&e.children, Some(e), deeper, out);
            }
        }
    }
}

/// Ячейка ли это — по тегу или по стилю.
/// Безымянный элемент починки таблицы: пустой стиль, только тег и дети.
/// Красится ли коробка (фон или рамка) — такой блок в бюджете строк
/// прячется целиком, если точка среза попала внутрь него.
pub(crate) fn has_box_style_probe(c: &Computed) -> bool {
    c.background.is_some()
        || c.bg_image.is_some()
        || c.gradient_raw.is_some()
        || c.border_visible.contains(&Some(true))
}

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
pub(crate) fn anon_role(n: &Node) -> Option<bool> {
    let Node::Element(e) = n else { return None };
    if e.style.float.is_some_and(|f| f != 0)
        || matches!(
            e.style.position,
            Some(crate::style::computed::Position::Absolute) | Some(crate::style::computed::Position::Fixed)
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
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО: считать табличной ролью и ПОДПИСЬ (§17.2.1),
    // чтобы бесхозный `display: table-caption` попадал в анонимную таблицу
    // (`e.style.is_caption == Some(true)` и тег `caption`). Срез из 18 пар с
    // подписью: 17 зелёных до и после, `caption-position-001` ушла
    // 2.79 → 2.88. Значит её держит не сборка анонимной таблицы.
    match e.style.display {
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
/// Поэлементная обёртка у нас уже была, и каждая бесхозная роль получала
/// СВОЮ таблицу — ряды вставали друг под друга отдельными таблицами вместо
/// одной. Починку содержимого (ряд вокруг ячеек, ячейка вокруг прочего)
/// делает `fixup_table_children` уже внутри собранной таблицы.
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
        if e.style.display.is_none()
            && e.inline
            && e.children.iter().any(|c| anon_role(c).is_some())
        {
            e.children = wrap_anon_tables_as(&e.children, Display::InlineTable);
        }
    }
}

pub(crate) fn wrap_anon_tables_as(nodes: &[Node], display: Display) -> Vec<Node> {
    if !nodes.iter().any(|n| anon_role(n).is_some()) {
        return nodes.to_vec();
    }
    let flush = |run: &mut Vec<Node>, out: &mut Vec<Node>| {
        if run.is_empty() {
            return;
        }
        let mut t = anon_element("table", std::mem::take(run));
        t.style.display = Some(display);
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

pub(crate) const ANON_CELL: &str = "anonymous-cell";

/// An HTML `td`/`th` element, whose UA style inherits the row's
/// `vertical-align` (HTML §15.3.9); anonymous cells keep the initial value.
pub(crate) fn html_cell(cell: &Element) -> bool {
    matches!(cell.tag.as_str(), "td" | "th") && cell.attr(ANON_CELL).is_none()
}

pub(crate) fn anon_element(tag: &str, children: Vec<Node>) -> Element {
    // CSS 2.1 §17.2.1: an anonymous cell is not an HTML `td`; the UA rule
    // `td { vertical-align: inherit }` (HTML §15.3.9) does not reach it.
    let attrs = if tag == "td" {
        vec![(ANON_CELL.into(), "1".into())]
    } else {
        vec![]
    };
    Element {
        list_item: None,
        node_id: 0,
        anim: None,
        tag: tag.into(),
        style: Computed::default(),
        hover: None,
        first_letter: None,
        first_line: None,
        children,
        attrs,
        inline: false,
    }
}

/// Починка детей таблицы (css-tables-3 §3): `display: contents` растворить,
/// бесхозные ячейки и непустой текст завернуть в анонимный ряд.
/// Чинит СОДЕРЖИМОЕ ряда (css-tables-3 §fixup): `display: contents`
/// растворяется с наследованием, последовательные не-ячейки сливаются в
/// одну анонимную ячейку, а вложенный ряд выталкивается ОТДЕЛЬНЫМ рядом
/// после текущего.
pub(crate) fn fixup_row_children(row: &Element) -> Vec<Node> {
    fn walk(
        nodes: &[Node],
        donor: Option<&Computed>,
        cells: &mut Vec<Node>,
        run: &mut Vec<Node>,
        extra: &mut Vec<Node>,
    ) {
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
                    walk(&merged, donor, cells, run, extra);
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
        let _ = donor;
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
        _ => false,
    });
    if !needs_fix {
        return vec![Node::Element(row.clone())];
    }
    let (mut cells, mut run, mut extra) = (vec![], vec![], vec![]);
    walk(&row.children, None, &mut cells, &mut run, &mut extra);
    table_roles::flush_inline(&mut cells, &mut run);
    let mut fixed = row.clone();
    fixed.children = cells;
    let mut out = vec![Node::Element(fixed)];
    out.extend(extra);
    out
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
                            } else if is_cell(&ge) {
                                stray.push(Node::Element(ge));
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
            _ => {}
        }
    }
    flush(&mut stray, &mut out);
    out
}

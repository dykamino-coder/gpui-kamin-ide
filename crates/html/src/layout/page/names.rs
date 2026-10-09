//! Имена страниц (`page`).
// owner: A

use crate::dom::{Element, Node};
use crate::layout::fragment::flex_lines::{class_a_box, item_container};
use crate::layout::fragment::probe::forced_opaque;
use crate::layout::fragment::table_bands::table_box;
use crate::render::{is_blank, out_of_flow};
use crate::style::computed::Display;
use crate::style::values::value::Len;

/// Начальное и конечное значения 'page' коробки (css-page-3 §"Using named
/// pages", п. 1-2): `auto` берёт имя ближайшего предка; начальное — от
/// ПЕРВОЙ дочерней коробки, конечное — от ПОСЛЕДНЕЙ, рекурсивно, но
/// передаёт значение только коробка, к которой свойство применяется
/// (класс A); текст, строчный, флоат, абсолют — не передают, и тогда
/// берётся используемое значение самой коробки.
pub(crate) fn page_names(e: &Element, inherited: &str) -> (String, String) {
    let used = e.style.page.clone().unwrap_or_else(|| inherited.to_string());
    // Крайняя дочерняя коробка — крайняя ПОТОЧНАЯ: абсолют и флоат в
    // точках класса A не участвуют (Blink берёт имя первого уложенного
    // поточного ребёнка, `SetPageNameIfNeeded`; `page-name-propagated-005`:
    // абсолют последним ребёнком не возвращал имя самой коробки).
    let boxes: Vec<&Node> = e
        .children
        .iter()
        .filter(|n| !is_blank(n))
        .filter(|n| !matches!(n, Node::Element(k) if matches!(k.style.display, Some(Display::None))
            || out_of_flow(&k.style) || k.style.float.unwrap_or(0) != 0))
        .collect();
    let via = |n: Option<&&Node>| match n {
        Some(Node::Element(k)) if !item_container(e) && class_a_box(k) => {
            Some(page_names(k, &used))
        }
        _ => None,
    };
    let start = via(boxes.first()).map(|p| p.0).unwrap_or_else(|| used.clone());
    let end = via(boxes.last()).map(|p| p.1).unwrap_or_else(|| used.clone());
    (start, end)
}

/// Объявления листа для марджин-боксов: контекст страницы (наследуемое
/// идёт в коробки, css-page-3 §page-properties) и коробки по именам.
pub type PageMarginDecls = (Vec<(String, String)>, Vec<(String, Vec<(String, String)>)>);

pub type PageMarginDeclsFn = std::rc::Rc<dyn Fn(usize, &str) -> PageMarginDecls>;

/// Используемое значение 'page' (css-page-3 §using-named-pages: `auto` —
/// значение ближайшего предка с не-`auto`) — в `style.page` каждого
/// элемента, чтобы мера фрагментации сравнивала имена на любой глубине.
pub(super) fn fill_used_page(nodes: &mut [Node], inherited: &str) {
    for n in nodes.iter_mut() {
        if let Node::Element(e) = n {
            if e.style.page.is_none() && !inherited.is_empty() {
                e.style.page = Some(inherited.to_string());
            }
            let used = e.style.page.clone().unwrap_or_default();
            fill_used_page(&mut e.children, &used);
        }
    }
}

/// Есть ли внутри коробки смена имени страницы между соседями класса A
/// (css-page-3 §using-named-pages п. 4) — на любой глубине.
fn renames_inside(e: &Element) -> bool {
    let kids: Vec<&Element> = e
        .children
        .iter()
        .filter_map(|n| match n {
            Node::Element(k) if class_a_box(k) => Some(k),
            _ => None,
        })
        .collect();
    (!item_container(e)
        && kids
            .windows(2)
            .any(|w| page_names(w[0], "").1 != page_names(w[1], "").0))
        || kids.iter().any(|k| renames_inside(k))
}

/// Есть ли внутри коробки принудительный разрыв МЕЖДУ соседями класса A
/// (css-break-4 §3.1 `break-before`/`break-after` не у крайнего ребёнка;
/// крайний передаёт разрыв самой коробке, `edge_break`) — на любой глубине
/// блочного потока. Мера коробки с текстом неизвестна (`shape_full` —
/// `None`), и разрыв внутри такого ребёнка стопки иначе терялся
/// (`page-name-propagated-002-print-ref`: `break-before: page` у второго
/// ребёнка обёртки).
fn breaks_inside(e: &Element) -> bool {
    // Только блочный поток: внутри таблицы разрыв режет ряды и группы
    // (`rowgroup-page-break-inside-avoid-5-print-ref`: `thead { break-after }`
    // — таблица не обёртка, снимать её нельзя).
    let table_part = matches!(
        e.tag.as_str(),
        "table" | "thead" | "tbody" | "tfoot" | "tr" | "td" | "th" | "caption" | "colgroup"
    );
    if table_part || table_box(e) || item_container(e) || forced_opaque(e) {
        return false;
    }
    let kids: Vec<&Element> = e
        .children
        .iter()
        .filter_map(|n| match n {
            Node::Element(k) if class_a_box(k) => Some(k),
            _ => None,
        })
        .collect();
    let n = kids.len();
    kids.iter().enumerate().any(|(i, k)| {
        (i > 0 && k.style.break_before_force)
            || (i + 1 < n && k.style.break_after_force)
            || breaks_inside(k)
    })
}

/// Обёртка без собственной коробки на листе: блок без полей, рамок,
/// отбивок, фона, размеров, разрывов и прочего, что видно или влияет на
/// раскладку детей. Снятие такой обёртки раскладку не меняет.
fn plain_wrapper(e: &Element) -> bool {
    let zero = |l: &Option<Len>| matches!(l, None | Some(Len::Px(0.0)));
    let st = &e.style;
    let b = st.borders();
    !e.inline
        && matches!(st.display, None | Some(Display::Block))
        && st.position.is_none()
        && st.float.unwrap_or(0) == 0
        && [&st.margin.top, &st.margin.right, &st.margin.bottom, &st.margin.left]
            .iter()
            .all(|l| zero(l))
        && [&st.padding.top, &st.padding.right, &st.padding.bottom, &st.padding.left]
            .iter()
            .all(|l| zero(l))
        && [&b.top, &b.right, &b.bottom, &b.left].iter().all(|l| zero(l))
        && st.background.is_none_or(|c| c.a == 0.0)
        && st.bg_image.is_none()
        && st.width.is_none()
        && st.height.is_none()
        && st.min_width.is_none()
        && st.min_height.is_none()
        && st.max_width.is_none()
        && st.max_height.is_none()
        && st.overflow_x.is_none()
        && st.overflow_y.is_none()
        && st.opacity.is_none()
        && st.transform.is_none()
        && st.filter.is_none()
        && st.outline.is_none()
        && st.column_count.is_none()
        && st.z_index.is_none()
        && st.vertical != Some(true)
        && !st.break_before_force
        && !st.break_after_force
        && e.children.iter().filter(|n| !is_blank(n)).all(|n| matches!(n, Node::Element(_)))
}

/// Снимает простые обёртки, внутри которых меняется имя страницы: их дети
/// становятся детьми стопки, и разрыв по смене имени (css-page-3
/// §using-named-pages п. 4) ставится между ними, как между детьми корня.
/// Мера фрагментации (`shape_full`) у коробок с текстом неизвестна, и
/// разрыв внутри такого ребёнка стопки иначе не ставится вовсе.
pub(super) fn hoist_named_wrappers(nodes: &mut Vec<Node>) {
    loop {
        let mut changed = false;
        let mut out = Vec::with_capacity(nodes.len());
        for n in std::mem::take(nodes) {
            match n {
                Node::Element(e) if plain_wrapper(&e) && (renames_inside(&e) || breaks_inside(&e)) => {
                    changed = true;
                    out.extend(e.children);
                }
                n => out.push(n),
            }
        }
        *nodes = out;
        if !changed {
            break;
        }
    }
}

/// Имя ПЕРВОЙ страницы (css-page-3 §using-named-pages, п. 3): start value
/// первой поточной коробки класса A детей корня, иначе имя самого корня.
pub(super) fn first_kid_page_name(nodes: &[Node], root_page: &str) -> String {
    for n in nodes.iter().filter(|n| !is_blank(n)) {
        match n {
            Node::Element(e) if matches!(e.style.display, Some(Display::None)) => continue,
            Node::Element(e) if class_a_box(e) => return page_names(e, root_page).0,
            Node::Element(e) if !e.inline => continue,
            _ => return root_page.to_string(),
        }
    }
    root_page.to_string()
}

/// Имя первой страницы документа — для стенда: геометрия первого листа даёт
/// начальный содержащий блок (css-page-3 §page-model).
pub fn first_page_name(nodes: &[Node]) -> String {
    let mut nodes: Vec<Node> = nodes.to_vec();
    let mut root_page = String::new();
    loop {
        let live: Vec<&Node> = nodes.iter().filter(|n| !is_blank(n)).collect();
        let [Node::Element(e)] = live.as_slice() else { break };
        if !matches!(e.tag.as_str(), "html" | "body") {
            break;
        }
        if let Some(p) = &e.style.page {
            root_page = p.clone();
        }
        nodes = (*e).clone().children;
    }
    first_kid_page_name(&nodes, &root_page)
}

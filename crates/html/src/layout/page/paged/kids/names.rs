//! Имена страниц на разрывах между группами детей стопки страниц.

use crate::document::state::ValueCx;
use crate::dom::Node;
use crate::layout::fragment::breaks::{edge_break, page_monolith};
use crate::layout::fragment::flex_lines::{class_a_box, inline_display};
use crate::layout::page::names::page_names;
use crate::render::is_blank;

pub(super) fn group_break_names(
    root_page: &str,
    prev_end: &mut Option<String>,
    floated: impl Fn(&Node) -> bool,
    group: &[Node],
    n: &Node,
    vcx: ValueCx<'_>,
) -> (bool, bool, bool, bool, String) {
    let (monolith, fb, fa, names) = match n {
        Node::Element(e) if class_a_box(e) => (
            page_monolith(e),
            edge_break(e, false, vcx),
            edge_break(e, true, vcx),
            Some(page_names(e, root_page)),
        ),
        Node::Element(e) if !e.inline && !inline_display(e) => (
            page_monolith(e),
            edge_break(e, false, vcx),
            edge_break(e, true, vcx),
            None,
        ),
        _ => (
            false,
            false,
            false,
            Some((root_page.to_string(), root_page.to_string())),
        ),
    };
    // Группа флоата: сам флоат имени не передаёт (§named pages п. 2), но
    // поточные коробки класса A в группе — передают конец — у последней (`page-name-000-print`: флоат, `clear`-блок
    // страницы `foo` и следом блок страницы `bar` — разрыв перед `bar`).
    let names = match (&names, n) {
        (None, Node::Element(e)) if floated(n) && !class_a_box(e) => {
            let named: Vec<(String, String)> = group
                .iter()
                .filter_map(|g| match g {
                    Node::Element(k) if class_a_box(k) => Some(page_names(k, root_page)),
                    _ => None,
                })
                .collect();
            // Начало группы — продолжение предыдущей: разрыв перед флоатом
            // увёл бы и его (`page-name-float-002-print`: флоат `b` остаётся
            // на листе `a`).
            let start = prev_end.clone().unwrap_or_else(|| root_page.to_string());
            named.last().map(|l| (start, l.1.clone()))
        }
        _ => names,
    };
    // Группа из нескольких узлов монолитом не бывает: её режет край листа.
    let monolith = monolith && group.iter().filter(|g| !is_blank(g)).count() == 1;
    let renamed = match (&*prev_end, &names) {
        (Some(p), Some((start, _))) => p != start,
        _ => false,
    };
    // Имя, с которого коробка начинается: своё у коробки класса A, иначе
    // — конец предыдущей (анонимный блок и прочие продолжают страницу).
    let start_name = match &names {
        Some((start, _)) => start.clone(),
        None => prev_end.clone().unwrap_or_else(|| root_page.to_string()),
    };
    if let Some((_, end)) = &names {
        *prev_end = Some(end.clone());
    }
    (fb, fa, monolith, renamed, start_name)
}

//! Группы строчных прогонов и боковые поля обёртки.

use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::table::anon::anon_element;
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div};

/// Копия ребёнка стопки встаёт КОРНЕМ (`layout_as_root` во всю
/// колонку), а корень taffy своих полей не кладёт: боковое поле `margin: 0 1em`
/// пропадало, текст ложился от края колонки (`multicol-nested-002`). Обёртка-
/// колонка делает копию обычным ребёнком: её поля и растяжение решает
/// раскладка (CSS 2.1 §10.3.3). Только горизонтальная стопка и только при
/// ненулевых полях в точках — иначе копия прежняя.
pub(crate) fn side_margin_wrap(el: AnyElement, copy: &Element, vertical: bool) -> AnyElement {
    let nz = |l: &Option<Len>| matches!(l, Some(Len::Px(v)) if v.abs() > 0.001);
    if vertical
        || !(nz(&copy.style.computed().margin.left) || nz(&copy.style.computed().margin.right))
    {
        return el;
    }
    div()
        .flex()
        .flex_col()
        .w_full()
        .child(el)
        .into_any_element()
}

/// Строчные прогоны среди блочных детей многоколоночника — в анонимные блоки
/// (CSS 2.1 §9.2.1.1: «If a block container box has a block-level box inside
/// it, then we force it to have only block-level boxes inside it» — строчное
/// содержимое оборачивается анонимной блочной коробкой). Тогда стопка колонок
/// видит их обычными детьми и режет по строкам. `None` — заворачивать нечего.
pub(crate) fn group_inline_runs(e: &Element) -> Option<Element> {
    let inline_level = |n: &Node| match n {
        Node::Text(_) => true,
        Node::Element(k) => {
            k.inline && !out_of_flow(&k.style.computed()) && k.style.computed().display.is_none()
        }
    };
    if !e.children.iter().any(|n| inline_level(n) && !is_blank(n)) {
        return None;
    }
    let mut out: Vec<Node> = Vec::new();
    let mut run: Vec<Node> = Vec::new();
    let flush = |run: &mut Vec<Node>, out: &mut Vec<Node>| {
        if run.iter().all(is_blank) {
            out.append(run);
        } else {
            out.push(Node::Element(anon_element(
                "anon-block",
                std::mem::take(run),
            )));
        }
    };
    for n in &e.children {
        if inline_level(n) {
            run.push(n.clone());
        } else {
            flush(&mut run, &mut out);
            out.push(n.clone());
        }
    }
    flush(&mut run, &mut out);
    // `text-box-trim` хоста режет его ПЕРВУЮ/ПОСЛЕДНЮЮ отформатированную
    // строку (css-inline-3 §4.2). Строки ушли в анонимные блоки — флаг едет
    // туда, где строка: первому анонимному, если он первый ребёнок, и
    // последнему, если последний (`text-box-trim-multicol-001`).
    if e.style.computed().text_box_trim_start || e.style.computed().text_box_trim_end {
        let flow: Vec<usize> = out
            .iter()
            .enumerate()
            .filter(|(_, n)| !is_blank(n))
            .map(|(i, _)| i)
            .collect();
        let anon = |n: &Node| matches!(n, Node::Element(k) if k.tag == "anon-block");
        if e.style.computed().text_box_trim_start
            && let Some(&i) = flow.first()
            && anon(&out[i])
            && let Node::Element(k) = &mut out[i]
        {
            k.style.used_mut().text_box_trim_start = true;
        }
        if e.style.computed().text_box_trim_end
            && let Some(&i) = flow.last()
            && anon(&out[i])
            && let Node::Element(k) = &mut out[i]
        {
            k.style.used_mut().text_box_trim_end = true;
        }
    }
    let mut g = e.clone();
    g.children = out;
    Some(g)
}

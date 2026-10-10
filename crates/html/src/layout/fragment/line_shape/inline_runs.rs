//! Группы строчных прогонов, транспонирование дерева и боковые поля обёртки.

use crate::dom::{Element, Node};
use crate::layout::table::anon::anon_element;
use crate::render::{is_blank, out_of_flow};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div};

/// Копия ребёнка стопки встаёт КОРНЕМ (`flow.rs` `layout_as_root` во всю
/// колонку), а корень taffy своих полей не кладёт: боковое поле `margin: 0 1em`
/// пропадало, текст ложился от края колонки (`multicol-nested-002`). Обёртка-
/// колонка делает копию обычным ребёнком: её поля и растяжение решает
/// раскладка (CSS 2.1 §10.3.3). Только горизонтальная стопка и только при
/// ненулевых полях в точках — иначе копия прежняя.
pub(crate) fn side_margin_wrap(el: AnyElement, copy: &Element, vertical: bool) -> AnyElement {
    let nz = |l: &Option<Len>| matches!(l, Some(Len::Px(v)) if v.abs() > 0.001);
    if vertical || !(nz(&copy.style.margin.left) || nz(&copy.style.margin.right)) {
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
        Node::Element(k) => k.inline && !out_of_flow(&k.style) && k.style.display.is_none(),
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
    if e.style.text_box_trim_start || e.style.text_box_trim_end {
        let flow: Vec<usize> = out
            .iter()
            .enumerate()
            .filter(|(_, n)| !is_blank(n))
            .map(|(i, _)| i)
            .collect();
        let anon = |n: &Node| matches!(n, Node::Element(k) if k.tag == "anon-block");
        if e.style.text_box_trim_start
            && let Some(&i) = flow.first()
            && anon(&out[i])
            && let Node::Element(k) = &mut out[i]
        {
            k.style.text_box_trim_start = true;
        }
        if e.style.text_box_trim_end
            && let Some(&i) = flow.last()
            && anon(&out[i])
            && let Node::Element(k) = &mut out[i]
        {
            k.style.text_box_trim_end = true;
        }
    }
    let mut g = e.clone();
    g.children = out;
    Some(g)
}

/// Клон поддерева, у которого ФИЗИЧЕСКИЕ поля коробки повёрнуты так, что
/// БЛОЧНАЯ ось вертикального письма встаёт на место вертикальной: `width` ↔
/// `height`, стороны — по логическим ролям (css-writing-modes-4 §3.1, §6.4
/// «abstract-to-physical mappings»): новый верх — block-start (левый край у
/// `vertical-lr`, правый у `vertical-rl`), новый низ — block-end, новые лево/право
/// — inline-start/-end (верх/низ при `direction: ltr`).
///
/// Нужен ТОЛЬКО мере стопки колонок: `shape_full` написана в терминах блочного
/// потока (`h` — размер по оси потока, `cuts`/`solid` — смещения от его начала),
/// но читает физические поля. На повёрнутом клоне её `h` — блочный размер, а
/// `flex-direction: row` остаётся строчной осью (в вертикали она вертикальна) —
/// дети ряда стоят рядом, точек разреза между ними нет, как и должно быть.
/// Рисуется по-прежнему ИСХОДНЫЙ элемент: повернуть отрисовку нельзя, вместе с
/// коробкой повернулись бы текст, рамки и фон. Blink делает то же логическими
/// величинами (`BoxStrut`/`LogicalSize` в `block_layout_algorithm.cc`).
///
/// `None` — в поддереве потомок с ДРУГИМ письмом (ортогональный поток,
/// css-writing-modes-4 §7.3, или обратная блочная ось) либо `direction: rtl`:
/// поворотом его мера не выражается, и многоколоночник остаётся на прежнем
/// пути.
pub(crate) fn transpose_tree(c: &Element, rl: bool) -> Option<Element> {
    if c.style.vertical == Some(false)
        || c.style.vertical_rl.is_some_and(|v| v != rl)
        || c.style.rtl == Some(true)
    {
        return None;
    }
    let turn = |s: &crate::style::computed::Sides| crate::style::computed::Sides {
        top: if rl { s.right } else { s.left },
        bottom: if rl { s.left } else { s.right },
        left: s.top,
        right: s.bottom,
    };
    let mut t = c.clone();
    std::mem::swap(&mut t.style.width, &mut t.style.height);
    std::mem::swap(&mut t.style.min_width, &mut t.style.min_height);
    std::mem::swap(&mut t.style.max_width, &mut t.style.max_height);
    t.style.padding = turn(&c.style.padding);
    t.style.margin = turn(&c.style.margin);
    t.style.border_width = turn(&c.style.border_width);
    t.style.inset = turn(&c.style.inset);
    // Видимость рамки — `[верх, право, низ, лево]` (`Computed::borders`).
    let v = c.style.border_visible;
    t.style.border_visible = if rl {
        [v[1], v[2], v[3], v[0]]
    } else {
        [v[3], v[2], v[1], v[0]]
    };
    // Обрезка ПО ОСИ ПОТОКА: в вертикальном письме это `overflow-x`.
    t.style.overflow_y = c.style.overflow_x;
    t.style.overflow_x = c.style.overflow_y;
    // `border-spacing` физическое (`horizontal vertical`), ряды таблицы идут
    // по оси потока: между рядами в вертикали — ГОРИЗОНТАЛЬНАЯ составляющая.
    if let Some((x, y)) = c.style.border_spacing {
        t.style.border_spacing = Some((y, x));
    }
    t.children = c
        .children
        .iter()
        .map(|n| match n {
            Node::Element(k) => transpose_tree(k, rl).map(Node::Element),
            other => Some(other.clone()),
        })
        .collect::<Option<Vec<Node>>>()?;
    Some(t)
}

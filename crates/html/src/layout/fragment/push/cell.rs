//! Перенос ячейки таблицы и рост перед перенесённой коробкой.

use super::pushed_box_at;
use crate::dom::{Element, Node};
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::table::anon::fixup_table_children;
use crate::layout::table::is_cell;
use crate::render::is_blank;
use crate::style::computed::Display;
use crate::style::values::value::Len;

/// Распорка в таблице по тегу: та же арифметика рядов, что у `table_shape`
/// (порядок групп `thead`/…/`tfoot`, `border-spacing`, презентационные
/// `cellspacing`/`cellpadding`, высота ряда — наибольшая мера ячейки), без
/// `avoid` и разрывов; у ряда, накрывающего `a`, — первая ячейка, в которой
/// нашлась коробка. Точка ровно на верху ряда — первый ребёнок ячейки
/// (ячейка — свой контекст, поле сквозь её верх не уходит; в мере
/// `shape_full(cell)` распорка ложится в `lead`). Что `table_shape` не
/// меряет (`rowspan`, подпись, сросшиеся рамки, заданная высота), здесь
/// тоже `None` — распорки нет, поведение прежнее.
pub(super) fn pushed_cell_at(c: &Element, a: f32, depth: u8) -> Option<u64> {
    if depth == 0
        || c.style.vertical == Some(true)
        || c.style.border_collapse == Some(true)
        || c.style.height.is_some()
        || c.style.min_height.is_some()
    {
        return None;
    }
    let px_of = |l: &Option<Len>| match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        _ => None,
    };
    let top = px_of(&c.style.padding.top)? + px_of(&c.style.borders().top)?;
    let attr_px = |name: &str| {
        c.attr(name)
            .and_then(|v| v.trim().trim_end_matches("px").parse::<f32>().ok())
    };
    let ua_default = matches!(
        c.style.border_spacing,
        Some((Some(Len::Px(2.0)), Some(Len::Px(2.0))))
    );
    let spacing = match (attr_px("cellspacing"), ua_default, &c.style.border_spacing) {
        (Some(v), true, _) | (Some(v), _, None) => v,
        (_, _, Some((_, y))) => px_of(y)?,
        _ => 0.0,
    };
    let cell_cx = ShapeCx {
        cell_pad: attr_px("cellpadding"),
        ..ShapeCx::COLUMNS
    };
    let is_row = |e: &Element| e.tag == "tr" || e.style.display == Some(Display::TableRow);
    let is_group = |e: &Element| {
        matches!(e.tag.as_str(), "thead" | "tbody" | "tfoot")
            || e.style.display == Some(Display::TableRowGroup)
            || e.style.row_group_kind.is_some()
    };
    // Щуп разреза ходит по ТОМУ ЖЕ дереву, что мера (Х2) и рисование: иначе
    // `table_shape` посчитает точки по анонимным рядам, а `pushed_cell_at` на
    // тех же детях вернёт `None`, и распорка роста (`grow_pushed`) не найдёт
    // коробку, которую надо дотянуть до низа колонки.
    let fixed = fixup_table_children(&c.children);
    let mut parts: Vec<(u8, &Element)> = Vec::new();
    let (mut head, mut foot) = (false, false);
    for n in fixed.iter().filter(|n| !is_blank(n)) {
        let Node::Element(e) = n else { return None };
        let role = match e.tag.as_str() {
            "thead" => Some(0u8),
            "tbody" => Some(1),
            "tfoot" => Some(2),
            _ => e.style.row_group_kind,
        };
        let kind = match role {
            Some(0) if !head => {
                head = true;
                0
            }
            Some(2) if !foot => {
                foot = true;
                2
            }
            _ if is_row(e) || is_group(e) => 1,
            _ => return None,
        };
        parts.push((kind, e));
    }
    parts.sort_by_key(|p| p.0);
    let mut rows: Vec<&Element> = Vec::new();
    for (_, e) in &parts {
        if is_row(e) {
            rows.push(e);
            continue;
        }
        for n in e.children.iter().filter(|n| !is_blank(n)) {
            match n {
                Node::Element(r) if is_row(r) => rows.push(r),
                _ => return None,
            }
        }
    }
    let mut y = top;
    for r in rows {
        let start = y + spacing;
        let mut h = px_of(&r.style.height)?;
        let mut cells: Vec<&Element> = Vec::new();
        for n in r.children.iter().filter(|n| !is_blank(n)) {
            let Node::Element(cell) = n else { return None };
            if !is_cell(cell) || cell.attr("rowspan").is_some_and(|v| v.trim() != "1") {
                return None;
            }
            h = h.max(shape_full(cell, depth - 1, cell_cx)?.0);
            cells.push(cell);
        }
        if a > start - 0.01 && a < start + h - 0.01 {
            return cells
                .iter()
                .find_map(|cell| pushed_box_at(cell, a - start, depth - 1));
        }
        y = start + h;
    }
    None
}

/// Распорка роста: `margin-top += grow` у потомка `id`. Поле ложится в
/// `lead` меры (`shape_full`) и в раскладку копии одинаково; недобор от
/// схлопывания с большим нижним полем соседа добирает следующий заход
/// `grow_pushed`.
pub(super) fn grow_before(c: &mut Element, id: u64, grow: f32) -> bool {
    for n in c.children.iter_mut() {
        let Node::Element(k) = n else {
            continue;
        };
        if k.node_id == id {
            let old = match &k.style.margin.top {
                Some(Len::Px(v)) => *v,
                _ => 0.0,
            };
            k.style.margin.top = Some(Len::Px(old + grow));
            return true;
        }
        if grow_before(k, id, grow) {
            return true;
        }
    }
    false
}

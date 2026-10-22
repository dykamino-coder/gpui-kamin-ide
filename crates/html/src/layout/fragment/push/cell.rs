//! Перенос ячейки таблицы и рост перед перенесённой коробкой.

use super::pushed_box_at;
use crate::document::state::ValueCx;
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::table_bands::parts::{cell_settings, is_row, ordered_parts};
use crate::layout::fragment::table_bands::row_geometry;
use crate::layout::table::anon::fixup_table_children;
use crate::style::values::value::Len;

/// Распорка в таблице по тегу: та же арифметика рядов, что у `table_shape`
/// (порядок групп `thead`/…/`tfoot`, `border-spacing`, презентационные
/// `cellspacing`/`cellpadding`, высота ряда — наибольшая мера ячейки), без
/// `avoid` и разрывов; у ряда, накрывающего `a`, — первая ячейка, в которой
/// нашлась коробка. Точка ровно на верху ряда — первый ребёнок ячейки
/// (ячейка — свой контекст, поле сквозь её верх не уходит; в мере
/// `shape_full(cell)` распорка ложится в `lead`). Охваты и высоты строк
/// берутся из общей `row_geometry`. Подпись, сросшиеся рамки и заданная высота здесь
/// тоже `None` — распорки нет, поведение прежнее.
pub(super) fn pushed_cell_at(
    c: &Element,
    a: f32,
    depth: u8,
    forced: bool,
    vcx: ValueCx<'_>,
) -> Option<u64> {
    if depth == 0
        || c.style.wm().vertical()
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
    let (spacing, cell_cx) = cell_settings(c, ShapeCx::COLUMNS)?;
    // Щуп разреза ходит по ТОМУ ЖЕ дереву, что мера и рисование: иначе
    // `table_shape` посчитает точки по анонимным рядам, а `pushed_cell_at` на
    // тех же детях вернёт `None`, и распорка роста (`grow_pushed`) не найдёт
    // коробку, которую надо дотянуть до низа колонки.
    let fixed = fixup_table_children(&c.children, vcx);
    let parts = ordered_parts(&fixed, |_| false)?;
    let mut rows: Vec<(usize, &Element)> = Vec::new();
    for (group, (_, e)) in parts.iter().enumerate() {
        if is_row(e) {
            rows.push((usize::MAX, e));
            continue;
        }
        for n in e.children.iter().filter(|n| !is_blank(n)) {
            match n {
                Node::Element(r) if is_row(r) => rows.push((group, r)),
                _ => return None,
            }
        }
    }
    let geometry = row_geometry(&rows, depth, cell_cx, spacing, vcx)?;
    let mut y = top;
    for row in geometry {
        let start = y + spacing;
        if a > start - 0.01 && a < start + row.height - 0.01 {
            return row
                .cells
                .iter()
                .filter(|cell| cell.expandable)
                .find_map(|cell| pushed_box_at(cell.cell, a - start, depth - 1, forced, vcx));
        }
        y = start + row.height;
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
            let old = crate::style::values::value::px_lossy_zero(k.style.margin.top);
            k.style.margin.top = Some(Len::Px(old + grow));
            return true;
        }
        if grow_before(k, id, grow) {
            return true;
        }
    }
    false
}

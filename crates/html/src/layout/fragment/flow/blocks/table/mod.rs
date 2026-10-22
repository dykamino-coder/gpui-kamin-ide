//! Таблица в блочной стопке копии: только факты стиля, без положений. Обёртка таблицы —
//! стопка верхних подписей, коробки таблицы и нижних подписей (css-tables-3 §terminology,
//! CSS 2.1 §17.4); коробка таблицы — ряды в порядке дорожек сетки (`layout::table::grid_rows`).
//! У ряда — принудительные разрывы и запреты на краях (свои, слитые со всеми ячейками ряда и
//! перенесённые с краёв группы: css-break-4 §break-between «Applies to … table row groups,
//! table rows», Blink `table_row_layout_algorithm.cc` `JoinFragmentainerBreakValues`),
//! `break-inside: avoid` ряда и группы, ячейки со своей блочной стопкой и охватом, первая
//! шапка и первый подвал (повтор, css-tables-3 §repeated-headers). Положения рядов даёт
//! разложенная сетка (`geometry`).

use super::{BlockBox, BlockKid};
use crate::document::state::ValueCx;
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::breaks::{edge_avoid, edge_break};
use crate::layout::fragment::clone::monolith_box;
use crate::layout::fragment::flow::box_children::children;
use crate::layout::table::grid_rows::{cell_row_span, grid_rows, table_key};
use crate::layout::table::is_cell;

mod edges;
mod geometry;
mod sections;
pub(crate) use sections::TableSections;
#[cfg(test)]
mod tests;

/// Таблица стопки: подписи сторон, ряды, секции повтора.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct TableBox {
    /// Ключ записи раскладки (`layout::table::grid_rows::table_key`).
    pub(crate) key: u64,
    pub(crate) caps_top: Vec<BlockKid>,
    pub(crate) caps_bot: Vec<BlockKid>,
    pub(crate) rows: Vec<TableRowBox>,
    /// Ряды первой шапки и первого подвала: `(первый, последний,
    /// break-inside: avoid секции)`.
    pub(crate) head: Option<(usize, usize, bool)>,
    pub(crate) foot: Option<(usize, usize, bool)>,
}

/// Ряд таблицы: факты стиля его границ и ячеек.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct TableRowBox {
    /// Номер куска рядов одной группы (подряд идущие ряды одной группы).
    pub(crate) group: usize,
    /// `break-inside: avoid*` группы ряда.
    pub(crate) group_avoid: bool,
    pub(crate) force_before: bool,
    pub(crate) force_after: bool,
    pub(crate) avoid_before: bool,
    pub(crate) avoid_after: bool,
    /// `break-inside: avoid*` самого ряда.
    pub(crate) avoid_inside: bool,
    /// Ячейки: охват по рядам и блочная стопка содержимого.
    pub(crate) cells: Vec<(usize, BlockBox)>,
}

impl TableBox {
    /// Потоки подписей и ячеек: их вложенные сетки тоже требуют записи
    /// дорожек, а принадлежность потомков не выводится из координат.
    pub(super) fn flows(&self) -> impl Iterator<Item = &BlockBox> {
        self.caps_top
            .iter()
            .chain(&self.caps_bot)
            .filter_map(|k| k.inner.as_ref())
            .chain(
                self.rows
                    .iter()
                    .flat_map(|r| r.cells.iter().map(|(_, b)| b)),
            )
    }

    /// Устройство таблицы `e` и все потоки её ячеек и подписей.
    pub(crate) fn collect(e: &Element, cx: ShapeCx, vcx: ValueCx<'_>) -> Self {
        let mut out = TableBox {
            key: table_key(e),
            ..Default::default()
        };
        for n in children(&e.children).filter(|n| !is_blank(n)) {
            let Node::Element(cap) = n else { continue };
            if cap.tag != "caption" && cap.style.is_caption != Some(true) {
                continue;
            }
            // Сторона — с самой подписи, при пустоте — с таблицы (как
            // `layout::table::finish::collect_captions`).
            let kid = caption_kid(cap, cx, vcx);
            if cap.style.caption_bottom.or(e.style.caption_bottom) == Some(true) {
                out.caps_bot.push(kid);
            } else {
                out.caps_top.push(kid);
            }
        }
        grid_rows(e, vcx, |rows| {
            let mut group = 0usize;
            for (i, r) in rows.iter().enumerate() {
                let same = |j: usize| {
                    rows.get(j).is_some_and(|o| match (o.group, r.group) {
                        (Some(a), Some(b)) => std::ptr::eq(a, b),
                        _ => false,
                    })
                };
                let first = i == 0 || !same(i - 1);
                let last = !same(i + 1);
                if first && i > 0 {
                    group += 1;
                }
                let g = r.group;
                let gs = |f: fn(&Element) -> bool| g.is_some_and(f);
                let cells = r
                    .row
                    .children
                    .iter()
                    .filter_map(|n| match n {
                        Node::Element(c) if is_cell(c) => {
                            Some((cell_row_span(c, r.left), BlockBox::container(c, cx, vcx)))
                        }
                        _ => None,
                    })
                    .collect();
                out.rows.push(TableRowBox {
                    group,
                    group_avoid: gs(|g| g.style.break_inside_avoid),
                    force_before: row_force(r.row, false, vcx)
                        || (first && gs(|g| g.style.break_before_force)),
                    force_after: row_force(r.row, true, vcx)
                        || (last && gs(|g| g.style.break_after_force)),
                    avoid_before: row_avoid(r.row, false, vcx)
                        || (first && gs(|g| g.style.break_before_avoid)),
                    avoid_after: row_avoid(r.row, true, vcx)
                        || (last && gs(|g| g.style.break_after_avoid)),
                    avoid_inside: r.row.style.break_inside_avoid,
                    cells,
                });
                let section = |slot: &mut Option<(usize, usize, bool)>| {
                    let avoid = match g {
                        Some(g) => g.style.break_inside_avoid,
                        None => r.row.style.break_inside_avoid,
                    };
                    *slot = Some(match *slot {
                        Some((a, _, _)) => (a, i, avoid),
                        None => (i, i, avoid),
                    });
                };
                if r.head {
                    section(&mut out.head);
                }
                if r.foot {
                    section(&mut out.foot);
                }
            }
        });
        out
    }

    /// Номера узлов коробок, которые таблица просит записать: подписи и
    /// блочные стопки подписей и ячеек.
    pub(crate) fn push_ids(&self, out: &mut Vec<u64>) {
        for k in self.caps_top.iter().chain(&self.caps_bot) {
            out.push(k.id);
            if let Some(inner) = &k.inner {
                inner.push_ids(out);
            }
        }
        for r in &self.rows {
            for (_, cell) in &r.cells {
                cell.push_ids(out);
            }
        }
    }

    /// Ключи этой и вложенных таблиц.
    pub(crate) fn push_keys(&self, out: &mut Vec<u64>) {
        out.push(self.key);
        for k in self.caps_top.iter().chain(&self.caps_bot) {
            if let Some(inner) = &k.inner {
                inner.push_keys(out);
            }
        }
        for r in &self.rows {
            for (_, cell) in &r.cells {
                cell.push_keys(out);
            }
        }
    }
}

/// Подпись — блочная коробка обёртки таблицы (CSS 2.1 §17.4: блочный
/// контейнер), сосед коробки таблицы и других подписей.
fn caption_kid(cap: &Element, cx: ShapeCx, vcx: ValueCx<'_>) -> BlockKid {
    let monolith = monolith_box(cap);
    BlockKid {
        id: cap.node_id,
        adjoins: true,
        force_before: edge_break(cap, false, vcx),
        force_after: edge_break(cap, true, vcx),
        avoid_before: edge_avoid(cap, false, vcx),
        avoid_after: edge_avoid(cap, true, vcx),
        monolith,
        inner: (!monolith).then(|| BlockBox::container(cap, cx, vcx)),
        page: None,
    }
}

/// `break-before`/`break-after: avoid*` ряда — своё значение и значения его
/// ячеек: ячейки ряда — параллельные потоки (css-break-4 §break-propagation
/// «parallel layout»), их значения сливаются в значение ряда (Blink
/// `table_row_layout_algorithm.cc` `JoinFragmentainerBreakValues`); с крайнего
/// ребёнка ячейки значение переносит `edge_avoid`.
pub(crate) fn row_avoid(row: &Element, last: bool, vcx: ValueCx<'_>) -> bool {
    edge_avoid(row, last, vcx)
        || row.children.iter().filter(|n| !is_blank(n)).any(
            |n| matches!(n, Node::Element(cell) if is_cell(cell) && edge_avoid(cell, last, vcx)),
        )
}

/// Принудительные `break-before`/`break-after` ряда — тем же слиянием с
/// ячейками (`break-before-expansion-001`: ячейка второго ряда с
/// `break-before: column`).
pub(crate) fn row_force(row: &Element, last: bool, vcx: ValueCx<'_>) -> bool {
    (if last {
        row.style.break_after_force
    } else {
        row.style.break_before_force
    }) || row
        .children
        .iter()
        .filter(|n| !is_blank(n))
        .any(|n| matches!(n, Node::Element(cell) if is_cell(cell) && edge_break(cell, last, vcx)))
}

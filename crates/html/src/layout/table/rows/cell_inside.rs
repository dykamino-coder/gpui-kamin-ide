//! Место ячейки в сетке (охваты, явные координаты) и её содержимое (BFC ячейки, обрезка, выравнивание).

use crate::dom::{Element, Node};
use crate::layout::block::margins::CELL_BFC;
use crate::layout::table::table_clipped_content;
use crate::render::{RenderOpts, blocks};
use crate::style::computed::{Align, Computed};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};

#[allow(clippy::too_many_arguments)]
pub(super) fn place_cell_in_grid(
    row_ix: i16,
    e: &Element,
    cols: u16,
    row_elements: &Vec<&Element>,
    col_ix: usize,
    span_cols: u16,
    span_rows: u16,
    mut d: gpui::Div,
) -> gpui::Div {
    // Вертикальное письмо таблицы: ряды идут ПОПЕРЁК — охваты
    // меняются осями вместе с сеткой (css-writing-modes-3 §8).
    let (grid_cols, grid_rows) = if e.style.vertical == Some(true) {
        (span_rows as u16, span_cols as u16)
    } else {
        (span_cols, span_rows as u16)
    };
    if grid_cols > 1 {
        d = d.col_span(grid_cols);
    }
    if grid_rows > 1 {
        d = d.row_span(grid_rows);
    }
    // Явные координаты вместо авто-потока: у `vertical-rl` ряды идут
    // от ПРАВОГО края, а авто-поток умеет только вперёд — реверс
    // рядов ломал охваты (замерено: -001 1.02 → 1.31, откачено).
    if e.style.vertical == Some(true) {
        let n_rows = row_elements.len() as i16;
        let gc = if e.style.vertical_rl == Some(true) {
            n_rows - row_ix - (span_rows as i16) + 2
        } else {
            row_ix
        };
        // Строчная ось вертикальной таблицы: `dir=rtl` разворачивает
        // её (ячейки снизу вверх), `text-orientation: upright`
        // ФОРСИРУЕТ ltr (§5.1 — upright задаёт направление ltr), а у
        // `sideways-lr` базовое направление само снизу вверх —
        // разворот инвертируется.
        let rtl_line = e.style.rtl == Some(true) && e.style.upright != Some(true);
        let base_up = e.style.sideways == Some(true) && e.style.vertical_rl != Some(true);
        let gr = if rtl_line != base_up {
            cols as i16 - col_ix as i16 - span_cols as i16 + 1
        } else {
            col_ix as i16 + 1
        };
        d = d.col_start(gc.max(1)).row_start(gr.max(1));
    } else if e.style.rtl == Some(true) {
        // `dir=rtl` на таблице: колонки идут от ПРАВОГО края
        // (CSS 2.2 §17.2) — та же явная расстановка, зеркалом.
        let gc = cols as i16 - col_ix as i16 - span_cols as i16 + 1;
        d = d.col_start(gc.max(1)).row_start(row_ix);
    } else {
        // Явная расстановка ВСЕГДА (CSS 2.1 §17.5.1: ячейка стоит в
        // ряду своего `<tr>` и в колонке по счёту с учётом охватов).
        // Авто-поток сетки рядов не знает: у ряда КОРОЧЕ прочих (одна
        // ячейка в столе из двух колонок) следующий ряд продолжал
        // заполнять ту же дорожку, и стол из `<thead>` «head» /
        // «body one» / «body two» / «foot» выходил «head body / one
        // body / two foot» (`rules-groups`, снимок s1234 против
        // эталона с явной расстановкой). Заодно порядок детей сетки
        // свободен для слоёв краски (см. `cells_over`).
        d = d.col_start(col_ix as i16 + 1).row_start(row_ix);
    }
    d
}

pub(super) fn cell_contents(
    opts: &RenderOpts,
    e: &Element,
    cm: Computed,
    spans_collapsed: bool,
    clipped: bool,
    cell: &Element,
    mut d: gpui::Div,
) -> (Vec<AnyElement>, gpui::Div) {
    // CSS 2.1 §9.4.1: a table cell establishes a block formatting
    // context, so its auto height contains its floats (§10.6.7). A
    // `td`/`th` gets its role from the tag and carries no `display`,
    // which `own_context_style` checks; without `CELL_BFC` its float
    // host took in-flow height only (`floats-wrap-bfc-001-right-
    // overflow`: the cell ended under the float's first 50px).
    CELL_BFC.with(|c| c.set(true));
    let inside = blocks(&cell.children, &cm, opts);
    CELL_BFC.with(|c| c.set(false));
    // Обрезанная ячейка не расталкивает колонки: её минимальный
    // вклад в дорожки НУЛЕВОЙ (css-sizing: automatic minimum при
    // overflow, отличном от visible, равен нулю) — иначе длинное
    // слово в обрезаемой объединённой ячейке раздавало ширину
    // колонкам, которых оно не должно касаться.
    if clipped {
        d = d.min_w(px(0.0));
    }
    let inside: Vec<AnyElement> = if spans_collapsed {
        // Ячейка через схлопнутую колонку: содержимое НЕ влияет на
        // ширины колонок вовсе (css-tables-3 §visibility-collapse) —
        // раскладка не должна его мерить, поэтому слой абсолютный.
        vec![
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .overflow_hidden()
                .children(inside)
                .into_any_element(),
        ]
    } else if clipped {
        vec![table_clipped_content::wrap(&mut d, inside)]
    } else if matches!(cm.vertical_align, Some(Align::Center) | Some(Align::End))
        && e.style.vertical != Some(true)
        && cell.children.iter().any(|n| {
            matches!(n, Node::Element(c) if matches!(
                c.style.position,
                Some(crate::style::computed::Position::Absolute)
                    | Some(crate::style::computed::Position::Fixed)
            ) && matches!(c.style.inset.top, None | Some(Len::Auto))
                && matches!(c.style.inset.bottom, None | Some(Len::Auto)))
        })
    {
        // CSS 2.1 §17.5.3 aligns the cell's IN-FLOW content; an
        // absolutely positioned child keeps the static position it
        // would have in that flow (§10.6.4). The cell aligns by flex
        // justification, which would centre the abspos box itself
        // (Flexbox §4.1) — align a wrapper of the contents instead,
        // whose height is the in-flow height only
        // (position-relative-table-*-left-absolute-child: HTML's
        // `vertical-align: middle` row groups lifted the box by half
        // its height).
        vec![
            div()
                .w_full()
                .flex()
                .flex_col()
                .children(inside)
                .into_any_element(),
        ]
    } else {
        inside
    };
    (inside, d)
}

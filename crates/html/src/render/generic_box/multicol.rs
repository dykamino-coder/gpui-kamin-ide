//! Выбор многоколоночного маршрута с сохранением ранних возвратов и состояния коробки.

use crate::layout::fragment::token::space::sequence::Nested;
use crate::layout::multicol::dimensions::MulticolDimensions;
use crate::layout::options::RenderOpts;
use std::rc::Rc;
mod spanners;
mod stack;
pub(super) use stack::stack_box;

use crate::dom::{Element, Node};
use crate::layout::multicol::column_flow::{column_flow, column_flow_with_dimensions};
use crate::layout::multicol::container::multicol_spanner_segments;
use crate::layout::multicol::measure::{RowPlan, row_plan, row_plan_in};
use crate::layout::multicol::spanner::{has_deep_spanner, spanner_box};
use crate::style::computed::{BoxStyle, Computed};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, px};

#[allow(clippy::too_many_arguments)]
pub(crate) fn multicol_box(
    d: gpui::Div,
    e: &Element,
    merged: BoxStyle,
    inherited: &Computed,
    opts: &RenderOpts,
    column_width: Option<Len>,
    cols: u16,
    used_gap: f32,
    col_axis: crate::layout::fragment::types::StackAxis,
    col_vert: bool,
    col_rl: bool,
    col_inline_size: Option<Len>,
    outer_space: Option<Rc<Nested>>,
    lone_span: bool,
) -> Result<(gpui::Div, BoxStyle), AnyElement> {
    if has_deep_spanner(e) {
        return Err(d
            .child(spanners::deferred(e, &merged, inherited, opts, outer_space))
            .into_any_element());
    }
    settled(
        d,
        e,
        merged,
        inherited,
        opts,
        column_width,
        cols,
        used_gap,
        col_axis,
        col_vert,
        col_rl,
        col_inline_size,
        outer_space,
        lone_span,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn settled(
    mut d: gpui::Div,
    e: &Element,
    merged: BoxStyle,
    inherited: &Computed,
    opts: &RenderOpts,
    column_width: Option<Len>,
    cols: u16,
    used_gap: f32,
    col_axis: crate::layout::fragment::types::StackAxis,
    col_vert: bool,
    col_rl: bool,
    col_inline_size: Option<Len>,
    outer_space: Option<Rc<Nested>>,
    lone_span: bool,
    rest_h: Option<f32>,
    dimensions: Option<(MulticolDimensions, MulticolDimensions)>,
) -> Result<(gpui::Div, BoxStyle), AnyElement> {
    // Сплошной текст режется на колонки по строкам, а не по детям:
    // один длинный абзац иначе оставался в первой колонке целиком.
    // `columns: auto <w>` без ширины коробки решается в замере —
    // туда уходит и число, и ширина колонки (§3.4).
    let RowPlan {
        col_w_px,
        want,
        col_h,
        box_h,
        row_gap,
        rows,
        nest_phase,
        nest_rows,
        outer_space,
    } = match dimensions {
        Some((_, own)) => row_plan_in(e, opts, column_width, cols, col_vert, outer_space, own),
        None => row_plan(e, &merged, opts, column_width, cols, col_vert, outer_space),
    };
    // Путь спаннера подготовлен native-фазой при разрешённой ширине.
    let span_prev: Option<Option<u64>> = None;
    let seg_open = false;
    let is_span = |n: &Node| matches!(n, Node::Element(c) if spanner_box(c));
    let unified = rows.is_some_and(|r| r.wrap && r.h.is_some()) && !opts.frag.in_stack_copy;
    if e.children.iter().any(&is_span) && !unified {
        return Err(multicol_spanner_segments(
            d, e, merged, opts, col_w_px, want, lone_span, rest_h, span_prev, seg_open, is_span,
        ));
    }
    // Текстовый путь `text-box-trim` не знает (ни у краёв колонок, ни у
    // первой/последней строки) — такой многоколоночник идёт стопкой
    // со строками (`line_run_shape`; `text-box-trim-multicol-009/010`).
    let trim_host = merged.text_box_trim_start || merged.text_box_trim_end;
    let flow = match dimensions {
        Some((dimensions, _)) => {
            column_flow_with_dimensions(e, &merged, opts, want, col_w_px, outer_space, dimensions)
        }
        None => column_flow(e, &merged, opts, want, col_w_px, outer_space),
    };
    if let Some(el) = flow.filter(|_| !trim_host) {
        // Коробка элемента остаётся своей: отступы и фон
        // принадлежат ей, поток живёт внутри.
        return Err(d.child(el).into_any_element());
    }
    if cols == 0 {
        // Число колонок при `columns: auto <w>` решается только в
        // замере текстового потока; блочный фоллбек — дорожками.
        if let Some(w) = col_w_px {
            d = d.grid().grid_cols_min(px(w));
        }
    } else {
        // Блочные дети с ИЗВЕСТНЫМИ высотами — честная укладка по
        // колонкам с балансом и монолитами (css-break, фаза 1).
        // Коробка ребёнка и его вертикальные поля отдельно:
        // поля схлопываются между соседями и на границах колонок.
        // Флоаты в поддереве ломают известность высоты.
        // Прямые абсолюты многоколоночника — не в стопку: их
        // содержащий блок — весь контейнер, рисуются его детьми
        // рядом со стопкой (`out-of-flow-in-multicolumn-094…097`
        // при нулевой записи в стопке уходили в колонку).
        // Статическая позиция (CSS 2.1 §10.6.4: «where the box
        // would have been if position were static»; §10.3.7 — то
        // же по строчной оси) — правило ПОЗИЦИОНИРОВАННОЙ
        // коробки. У плавающей своё место по §9.5, и щуп ей не
        // положен: нулевая запись в стопке меняет `kids.len()`, с
        // ним `balance_last` и весь план `fill_avoiding`
        // (`multicol-fill-balance-038` — монолитный флоат с
        // полями 40/70 при `margin-bottom: -30px` у соседа). Blink
        // перебирает в `LayoutFragmentainerDescendants` только
        // `oof_positioned_candidates`; флоат идёт обычной
        // укладкой (`PositionFloat`).
        return stack_box(
            d,
            e,
            merged,
            inherited,
            opts,
            cols,
            column_width,
            used_gap,
            row_gap,
            col_axis,
            col_vert,
            col_rl,
            col_h,
            box_h,
            col_inline_size,
            rows,
            nest_rows,
            nest_phase,
        );
    }
    Ok((d, merged))
}

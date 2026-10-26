//! Выбор многоколоночного маршрута с сохранением ранних возвратов и состояния коробки.

use crate::layout::fragment::token::space::sequence::Nested;
use crate::layout::options::RenderOpts;
use std::rc::Rc;
mod stack;
pub(super) use stack::stack_box;

use crate::dom::{Element, Node};
use crate::layout::multicol::column_flow::column_flow;
use crate::layout::multicol::container::multicol_spanner_segments;
use crate::layout::multicol::measure::{RowPlan, row_plan, tail_rest_height};
use crate::layout::multicol::spanner::{hoist_spanners, spanner_box};
use crate::style::computed::{BoxStyle, Computed};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, px};

#[allow(clippy::too_many_arguments)]
pub(crate) fn multicol_box(
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
) -> Result<(gpui::Div, BoxStyle), AnyElement> {
    let vcx = opts.cx.vcx();
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
    } = row_plan(e, &merged, opts, column_width, cols, col_vert, outer_space);
    // Спаннер среди инлайнового потока: режем детей на сегменты,
    // каждый сегмент — свой поток колонок, спаннер — блок между
    // ними (css-multicol §6). С рядами (`column-wrap: wrap` +
    // `column-height`) сегменты не годятся: у каждого свои ряды от
    // нуля, а Blink ведёт ОДИН курсор по коробке
    // (`column_layout_algorithm.cc` `intrinsic_block_size_`,
    // `LayoutSpanner`: спаннер, не влезший в остаток ряда, — со
    // следующего ряда; `column-height-006/013/017…020`). Такой
    // многоколоночник идёт единой стопкой, спаннер — её ребёнком
    // (`StackChild::span`). Внутри копии другой стопки — по-прежнему
    // сегментами: перенос ряда во внешнюю колонку не написан
    // (`column-height-029`).
    // Спаннер бывает НЕ прямым ребёнком: css-multicol-1
    // §column-span (`Overview.bs:1497-1499`) — «A spanning element
    // may be lower than the first level of descendants as long as
    // they are part of the same formatting context, and there is
    // nothing between the spanning element and multicol container
    // that establishes a containing block for fixed position
    // descendants». Спаннер выносится ИЗ ПОТОКА и режет
    // многоколоночник на «до», «спаннер во всю ширину» и «после»,
    // а его предки внутри многоколоночника разрезаются вместе с
    // ним. Поднимаем таких потомков к прямым детям ОДИН раз, до
    // всех решений ниже: дальше и сегментный путь, и единая
    // стопка, и текстовый `column_flow` видят спаннер прямым
    // ребёнком. Blink ведёт для этого путь `ColumnSpannerPath`
    // (`column_spanner_path.h`), у нас пути нет — предки режутся
    // прямо в дереве (`hoist_spanners`).
    // Сегментный путь ниже: метка родителя предыдущего спаннера,
    // если между ними не легло ни одного ряда колонок, и выложен
    // ли уже хоть один кусок (для сторожей полей).
    let span_prev: Option<Option<u64>> = None;
    let seg_open = false;
    let hoisted;
    let shape_cx = crate::layout::fragment::ShapeCx::COLUMNS.observed(opts);
    let e = match hoist_spanners(&e.children, shape_cx, vcx) {
        Some(kids) => {
            let mut c = e.clone();
            c.children = kids;
            hoisted = c;
            &hoisted
        }
        None => e,
    };
    let is_span = |n: &Node| matches!(n, Node::Element(c) if spanner_box(c));
    let unified = rows.is_some_and(|r| r.wrap && r.h.is_some()) && !opts.frag.in_stack_copy;
    // Высота хвостового ряда после единственного спаннера
    // (`tail_rest_height`, css-multicol-1 §column-fill).
    let rest_h: Option<f32> = tail_rest_height(e, &merged, box_h, rows, shape_cx, vcx);
    if e.children.iter().any(&is_span) && !unified {
        return Err(multicol_spanner_segments(
            d, e, merged, opts, col_w_px, want, lone_span, rest_h, span_prev, seg_open, is_span,
        ));
    }
    // Текстовый путь `text-box-trim` не знает (ни у краёв колонок, ни у
    // первой/последней строки) — такой многоколоночник идёт стопкой
    // со строками (`line_run_shape`; `text-box-trim-multicol-009/010`).
    let trim_host = merged.text_box_trim_start || merged.text_box_trim_end;
    if let Some(el) =
        column_flow(e, &merged, opts, want, col_w_px, outer_space).filter(|_| !trim_host)
    {
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

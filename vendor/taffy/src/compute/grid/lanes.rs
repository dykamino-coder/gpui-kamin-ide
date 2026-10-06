//! KaminIDE patch: раскладка ЛУНКАМИ (`display: grid-lanes`, css-grid-3).
//!
//! Спека — `refs/csswg-drafts/css-grid-3/Overview.bs`; референс — Blink
//! `third_party/blink/renderer/core/layout/grid_lanes/grid_lanes_layout_algorithm.cc`.
//!
//! Контейнер лунок — та же сетка, только с дорожками по ОДНОЙ оси («ось
//! решётки»). Дорожки этой оси размеряются ОБЩИМ алгоритмом css-grid-2 §12
//! (`track_sizing_algorithm`), а вкладываются в них «виртуальные» элементы:
//! явно размещённый — в свои дорожки, авто-размещаемый — во ВСЕ возможные
//! стартовые позиции (Overview.bs:619-669, Blink `BuildVirtualGridLanesItems`
//! :1785). По второй оси («ось укладки») решётки нет: каждый элемент встаёт в
//! лунку с наименьшей бегущей позицией в пределах порога `flow-tolerance`
//! (Overview.bs:885-968, Blink `RunGridLanesPlacementPhase` :1216).
//!
//! Прежде лунки строились в `crates/html` рукописной гибкой сеткой
//! (`render::lanes`), и размеры по оси решётки там ОЦЕНИВАЛИСЬ, а эталоны
//! (`grid-subgridded-to-grid-lanes/**` и соседи) рисовались настоящей сеткой
//! taffy: тест и эталон шли разными алгоритмами.
use super::OriginZeroLine;
use super::lanes_geometry::{absolute_area, stacking_alignment_offset, track_area};
use super::alignment::{align_item_within_area, align_tracks};
use super::explicit_grid::{
    AutoRepeatStrategy, compute_explicit_grid_size_in_axis, initialize_grid_tracks,
};
use super::subgrid;
use super::track_sizing::{
    determine_if_item_crosses_flexible_or_intrinsic_tracks, resolve_item_track_indexes,
    track_sizing_algorithm,
};
use super::types::{GridItem, GridTrack, NamedLineResolver, TrackCounts};
use crate::geometry::{AbsoluteAxis, AbstractAxis, InBothAbsAxis, Line, Point, Rect, Size};
use crate::style::{
    AlignItems, AlignItemsKeyword, AlignSelf, AlignmentSafety, AvailableSpace,
    Contain, Direction, GenericGridTemplateComponent, GenericRepetition, GridLanes, GridPlacement,
    MaxTrackSizingFunction, MinTrackSizingFunction, Overflow, Position, RepetitionCount,
    TrackSizingFunction,
};
use crate::style_helpers::*;
use crate::tree::{
    Baselines, Layout, LayoutInput, LayoutOutput, LayoutPartialTreeExt, NodeId, RunMode, SizingMode,
};
use crate::util::MaybeMath;
use crate::util::sys::{GridTrackVec, Vec, f32_max};
use crate::util::{MaybeResolve, ResolveOrZero};
use crate::{
    AlignContent, BoxGenerationMode, BoxSizing, CoreStyle, GridContainerStyle, GridItemStyle,
    LayoutGridContainer, LengthPercentage,
};

#[cfg(feature = "content_size")]
use crate::compute::common::scrollable_overflow::compute_scrollable_overflow_contribution;

/// Элемент лунок до размещения.
struct LaneChild {
    /// Узел.
    node: NodeId,
    /// Индекс среди детей контейнера.
    index: usize,
    /// Явная позиция по оси решётки — дорожки `[start, end)` в индексах
    /// дорожек неявной сетки (0 — первая дорожка); `None` — авто.
    definite: Option<(usize, usize)>,
    /// Пролёт по оси решётки.
    span: usize,
}

#[path = "lanes_baseline.rs"]
mod baseline_alignment;
#[path = "lanes_container_baselines.rs"]
mod container_baselines;
#[path = "lanes_final_placement.rs"]
mod final_placement;

/// Размещённый элемент.
struct Placed {
    baseline_x: Option<f32>,
    last_baseline_x: Option<f32>,
    baseline_x_flags: u8,
    node: NodeId,
    /// Дорожки по оси решётки.
    start: usize,
    end: usize,
    /// Смещение коробки (border-box) по оси решётки от начала контейнера.
    grid_pos: f32,
    /// Начало ВНЕШНЕЙ коробки по оси укладки от начала области содержимого.
    stack_pos: f32,
    /// Placement before content alignment/reversal and relative positioning.
    placement_stack_pos: f32,
    /// Размер border-box.
    size: Size<f32>,
    /// Поля (auto по оси укладки — ноль).
    margin: Rect<f32>,
    /// Сдвиг `position: relative` по оси укладки.
    stack_relative: f32,
    /// Внешний размер по оси укладки (без зазора).
    outer_stack: f32,
    padding: Rect<f32>,
    border: Rect<f32>,
    scrollbar_size: Size<f32>,
    #[cfg_attr(not(feature = "content_size"), allow(dead_code))]
    scrollable_overflow_rect: Rect<f32>,
    contain: Contain,
    #[cfg_attr(not(feature = "content_size"), allow(dead_code))]
    overflow: Point<Overflow>,
    baseline: Option<f32>,
    /// Выравнивание по оси укладки (css-grid-3 §stacking-self-alignment).
    stack_align: Option<AlignSelf>,
    /// Размер по оси укладки задан стилем — растяжка его не трогает
    /// (Blink :1067-1072).
    stack_size_fixed: bool,
    /// Область по оси решётки — для повторной раскладки при растяжке.
    area: f32,
    /// max-content размер по оси укладки (рядные лунки).
    stack_fit: Option<f32>,
    /// Выравнивание по базовой линии по оси решётки.
    grid_baseline: bool,
    /// KaminIDE patch: последняя базовая (от верха рамки) и участие в
    /// группе `last baseline` по оси решётки.
    last_baseline: Option<f32>,
    grid_last_baseline: bool,
}

/// Проём над элементом по оси укладки (Blink `TrackOpening`,
/// `grid_lanes_running_positions.cc:167-227`).
#[derive(Clone, Copy)]
struct Opening {
    start: f32,
    end: f32,
    /// Элемент НАД проёмом (его выравнивание этот проём и раздаёт).
    candidate: Option<usize>,
}

/// Результат раскладки одного элемента.
struct ItemBox {
    baseline_x: Option<f32>,
    last_baseline_x: Option<f32>,
    baseline_x_flags: u8,
    size: Size<f32>,
    margin: Rect<f32>,
    /// Смещение border-box внутри области по оси решётки.
    grid_offset: f32,
    stack_relative: f32,
    padding: Rect<f32>,
    border: Rect<f32>,
    scrollbar_size: Size<f32>,
    scrollable_overflow_rect: Rect<f32>,
    contain: Contain,
    overflow: Point<Overflow>,
    baseline: Option<f32>,
    stack_align: Option<AlignSelf>,
    stack_size_fixed: bool,
    grid_baseline: bool,
    last_baseline: Option<f32>,
    grid_last_baseline: bool,
}

/// Раскладка контейнера лунок.
pub(super) fn compute_grid_lanes_layout<Tree: LayoutGridContainer>(
    tree: &mut Tree,
    node: NodeId,
    inputs: LayoutInput,
    lanes: GridLanes,
) -> LayoutOutput {
    let LayoutInput {
        known_dimensions,
        parent_size,
        run_mode,
        ..
    } = inputs;
    let rows = lanes.rows;
    // Ось решётки — физическая: колонки (горизонталь) или ряды.
    let grid_abs = if rows {
        AbsoluteAxis::Vertical
    } else {
        AbsoluteAxis::Horizontal
    };
    let grid_axis = if rows {
        AbstractAxis::Block
    } else {
        AbstractAxis::Inline
    };

    let style = tree.get_grid_container_style(node);
    let available_space = super::lanes_stack_measure::container_space(style.size(), style.block_flow(), inputs);
    let flow = super::subgrid_flow::axes(&style);
    // Размеры контейнера — тем же порядком, что `compute_grid_layout`.
    let contain = style.contain();
    #[cfg(feature = "content_size")]
    let is_scroll_container =
        style.overflow().x.is_scroll_container() || style.overflow().y.is_scroll_container();
    let aspect_ratio = style.aspect_ratio();
    let padding = style
        .padding()
        .resolve_or_zero(parent_size.width, |val, basis| tree.calc(val, basis));
    let border = style
        .border()
        .resolve_or_zero(parent_size.width, |val, basis| tree.calc(val, basis));
    let padding_border = padding + border;
    let padding_border_size = padding_border.sum_axes();
    let box_sizing_adjustment = if style.box_sizing() == BoxSizing::ContentBox {
        padding_border_size
    } else {
        Size::ZERO
    };
    let min_size = style
        .min_size()
        .maybe_resolve(parent_size, |val, basis| tree.calc(val, basis))
        .maybe_apply_aspect_ratio(aspect_ratio)
        .maybe_add(box_sizing_adjustment);
    let max_size = style
        .max_size()
        .maybe_resolve(parent_size, |val, basis| tree.calc(val, basis))
        .maybe_apply_aspect_ratio(aspect_ratio)
        .maybe_add(box_sizing_adjustment);
    let preferred_size = if inputs.sizing_mode == SizingMode::InherentSize {
        style
            .size()
            .maybe_resolve(parent_size, |val, basis| tree.calc(val, basis))
            .maybe_apply_aspect_ratio(style.aspect_ratio())
            .maybe_add(box_sizing_adjustment)
    } else {
        Size::NONE
    };
    let scrollbar_gutter = style.overflow().transpose().map(|overflow| match overflow {
        Overflow::Scroll => style.scrollbar_width(),
        _ => 0.0,
    });
    let mut content_box_inset = padding_border;
    content_box_inset.right += scrollbar_gutter.x;
    content_box_inset.bottom += scrollbar_gutter.y;

    let align_content = style.align_content();
    let justify_content = style.justify_content();
    let align_items = style.align_items();
    let justify_items = style.justify_items();
    let align_content_safe = align_content.is_some_and(|a| a.safety == AlignmentSafety::Safe);
    let justify_content_safe = justify_content.is_some_and(|a| a.safety == AlignmentSafety::Safe);

    let constrained_available_space = known_dimensions
        .or(preferred_size)
        .map(|size| size.map(AvailableSpace::Definite))
        .unwrap_or(available_space)
        .maybe_clamp(min_size, max_size)
        .maybe_max(padding_border_size);
    let available_grid_space = Size {
        width: constrained_available_space
            .width
            .map_definite_value(|space| space - content_box_inset.horizontal_axis_sum()),
        height: constrained_available_space
            .height
            .map_definite_value(|space| space - content_box_inset.vertical_axis_sum()),
    };
    let outer_node_size = known_dimensions
        .or(preferred_size)
        .maybe_clamp(min_size, max_size)
        .maybe_max(padding_border_size);
    let mut inner_node_size = Size {
        width: outer_node_size
            .width
            .map(|space| space - content_box_inset.horizontal_axis_sum()),
        height: outer_node_size
            .height
            .map(|space| space - content_box_inset.vertical_axis_sum()),
    };
    if let (RunMode::ComputeSize, Some(width), Some(height)) =
        (run_mode, outer_node_size.width, outer_node_size.height)
    {
        return LayoutOutput::from_outer_size(Size { width, height });
    }

    // Явная сетка по оси решётки (css-grid-3 §grid-lanes-track-templates:
    // «formed in the same way as for a regular grid container»).
    // KaminIDE patch: рядные лунки с `aspect-ratio` и без своей высоты —
    // высота выводится из ширины (css-sizing-4 §5.1 «ratio-dependent axis»)
    // ещё до счёта повторов, и у повтора по рядам размер ОПРЕДЕЛЁН, а не
    // минимум: `aspect-ratio: 1/1; min-height: 60px; repeat(auto-fill, 50px)`
    // по рядам — ширина 60 (минимум, перенесённый соотношением), высота 60,
    // повтор один (эталон `row-auto-repeat-003-ref` — квадрат 60, тогда как
    // колонки того же вида дают 100: там повтор считается по ширине).
    let ratio_height = if rows && outer_node_size.height.is_none() && max_size.height.is_none() {
        aspect_ratio
            .zip(outer_node_size.width.or(min_size.width))
            .map(|(r, w)| (w / r).maybe_clamp(min_size.height, None))
    } else {
        None
    };
    let fit_basis = Size {
        width: outer_node_size.width,
        height: outer_node_size.height.or(ratio_height),
    };
    let auto_fit_container_size = fit_basis
        .or(max_size)
        .or(min_size)
        .maybe_clamp(min_size, max_size)
        .maybe_max(padding_border_size)
        .maybe_sub(content_box_inset.sum_axes());
    let auto_repeat_fit_strategy = fit_basis.or(max_size).map(|val| match val {
        Some(_) => AutoRepeatStrategy::MaxRepetitionsThatDoNotOverflow,
        None => AutoRepeatStrategy::MinRepetitionsThatDoOverflow,
    });
    let (col_auto_repetition_count, grid_template_col_count) = compute_explicit_grid_size_in_axis(
        &style,
        auto_fit_container_size.width,
        auto_repeat_fit_strategy.width,
        |val, basis| tree.calc(val, basis),
        AbsoluteAxis::Horizontal,
    );
    let (mut row_auto_repetition_count, mut grid_template_row_count) =
        compute_explicit_grid_size_in_axis(
            &style,
            auto_fit_container_size.height,
            auto_repeat_fit_strategy.height,
            |val, basis| tree.calc(val, basis),
            AbsoluteAxis::Vertical,
        );
    let (mut col_auto_repetition_count, mut grid_template_col_count) =
        (col_auto_repetition_count, grid_template_col_count);
    // Интрин-дорожки в `repeat(auto-*)` по оси решётки (css-grid-3 §7.2.1):
    // общий счёт сетки такой шаблон отвергает (нет фиксированной грани), а
    // лункам число повторов дают гипотетические размеры дорожек.
    let owned = owned_template(&style, grid_abs);
    let grid_gap_style = if rows {
        style.gap().height
    } else {
        style.gap().width
    };
    drop(style);
    if let Some(template) = owned {
        let avail = if rows {
            auto_fit_container_size.height
        } else {
            auto_fit_container_size.width
        };
        let strategy = if rows {
            auto_repeat_fit_strategy.height
        } else {
            auto_repeat_fit_strategy.width
        };
        let (reps, count) = intrinsic_repetitions(
            tree,
            node,
            &template,
            grid_gap_style,
            rows,
            avail,
            strategy,
            inner_node_size,
            align_items,
            justify_items,
        );
        if rows {
            row_auto_repetition_count = reps;
            grid_template_row_count = count;
        } else {
            col_auto_repetition_count = reps;
            grid_template_col_count = count;
        }
    }
    let style = tree.get_grid_container_style(node);
    let mut name_resolver =
        NamedLineResolver::new(&style, col_auto_repetition_count, row_auto_repetition_count);
    let explicit_col_count = grid_template_col_count.max(name_resolver.area_column_count());
    let explicit_row_count = grid_template_row_count.max(name_resolver.area_row_count());
    name_resolver.set_explicit_column_count(explicit_col_count);
    name_resolver.set_explicit_row_count(explicit_row_count);
    let explicit = if rows {
        explicit_row_count
    } else {
        explicit_col_count
    };

    // Зазор по оси укладки: «between the margin boxes of each pair of
    // adjacent items» (Overview.bs:1107-1116). Доля — от размера области
    // содержимого по этой же оси.
    let stack_gap_style = if rows {
        style.gap().width
    } else {
        style.gap().height
    };
    let stack_inner = if rows {
        inner_node_size.width
    } else {
        inner_node_size.height
    };
    let stack_gap =
        stack_gap_style.resolve_or_zero(stack_inner, |val, basis| tree.calc(val, basis));

    // Дети в потоке: позиция по оси решётки.
    let mut children: Vec<LaneChild> = Vec::new();
    let mut negative_implicit = 0u16;
    let mut positive_implicit = 0u16;
    let mut max_auto_span = 1u16;
    let mut raw_definite: Vec<Option<Line<OriginZeroLine>>> = Vec::new();
    for (index, child) in tree.child_ids(node).enumerate() {
        let child_style = tree.get_grid_child_style(child);
        if child_style.box_generation_mode() == BoxGenerationMode::None
            || child_style.position() == Position::Absolute
        {
            continue;
        }
        let placement = if rows {
            name_resolver.resolve_row_names(&child_style.grid_row())
        } else {
            name_resolver.resolve_column_names(&child_style.grid_column())
        }
        .map(|p| p.into_origin_zero_placement(explicit));
        if placement.is_definite() {
            let line = placement.resolve_definite_grid_lines();
            negative_implicit =
                negative_implicit.max(line.start.implied_negative_implicit_tracks());
            positive_implicit =
                positive_implicit.max(line.end.implied_positive_implicit_tracks(explicit));
            raw_definite.push(Some(line));
            children.push(LaneChild {
                node: child,
                index,
                definite: None,
                span: line.span().max(1) as usize,
            });
        } else {
            let span = placement.indefinite_span().max(1);
            max_auto_span = max_auto_span.max(span);
            raw_definite.push(None);
            children.push(LaneChild {
                node: child,
                index,
                definite: None,
                span: span as usize,
            });
        }
    }
    // §8.5 шаг 3.3: неявная сетка расширяется под самый широкий авто-пролёт.
    let total = negative_implicit + explicit + positive_implicit;
    if (max_auto_span) > total {
        positive_implicit += max_auto_span - total;
    }
    let counts = TrackCounts::from_raw(negative_implicit, explicit, positive_implicit);
    let n = counts.len();
    for (child, raw) in children.iter_mut().zip(raw_definite.iter()) {
        if let Some(line) = raw {
            let s = counts.oz_line_to_next_track(line.start).max(0) as usize;
            let e = (counts.oz_line_to_next_track(line.end).max(0) as usize)
                .min(n)
                .max(s + 1);
            child.definite = Some((s, e.min(n)));
        }
    }

    // `auto-fit` в лунках (Overview.bs:595-617): занятыми считаются дорожки
    // явно размещённых и первые N свободных, где N — сумма пролётов
    // авто-элементов; остальные дорожки повтора схлопываются.
    let mut occupied = Vec::with_capacity(n);
    occupied.resize(n, false);
    for child in &children {
        if let Some((s, e)) = child.definite {
            for flag in occupied.iter_mut().take(e).skip(s) {
                *flag = true;
            }
        }
    }
    // N считается от начала сетки, а не по свободным дорожкам: так делает Blink
    // (`BuildVirtualGridLanesItems`, :1866-1876: пролёт в диапазоне `auto-fit`
    // пропускается, если `EndLine() > unplaced_item_span_count`), и так ждёт
    // эталон `column-auto-repeat-auto-012` («the second track should still be
    // collapsed»: один авто-элемент, явные — в дорожках 1 и 3).
    let budget: usize = children
        .iter()
        .filter(|c| c.definite.is_none())
        .map(|c| c.span)
        .sum();
    for flag in occupied.iter_mut().take(budget) {
        *flag = true;
    }

    let mut grid_tracks: GridTrackVec<GridTrack> = GridTrackVec::new();
    initialize_grid_tracks(
        &mut grid_tracks,
        counts,
        &style,
        grid_abs,
        if rows {
            row_auto_repetition_count
        } else {
            col_auto_repetition_count
        },
        |i| occupied.get(i).copied().unwrap_or(false),
    );
    let collapsed = |tracks: &[GridTrack], t: usize| tracks[2 * t + 1].is_collapsed;

    let tolerance = match lanes.tolerance_pct {
        Some(k) => {
            k * if rows {
                inner_node_size.height
            } else {
                inner_node_size.width
            }
            .unwrap_or(0.0)
        }
        None => lanes.tolerance,
    };

    drop(style);

    // По оси укладки у КАЖДОГО элемента своя «дорожка»: в ней живёт оценка его
    // размера поперёк оси решётки. Для рядных лунок это max-content ширина
    // элемента (Blink `CreateConstraintSpaceForMeasure`, :3229-3236: «we have
    // to set the inline size as indefinite to allow for text flow», затем
    // `opt_fixed_inline_size = max_size`, :1332-1343); для колоночных —
    // неопределённая высота.
    let mut stack_tracks: GridTrackVec<GridTrack> = GridTrackVec::new();
    stack_tracks.push(GridTrack::gutter(LengthPercentage::length(0.0)));
    // max-content ширина border-box у элементов рядных лунок: по оси укладки
    // элемент меряется по содержимому, а не растягивается (у Blink та же мера
    // уходит в `opt_fixed_inline_size`, :1332-1343).
    let mut stack_max: Vec<Option<f32>> = Vec::with_capacity(children.len());
    // KaminIDE patch: подсетка в лунках (css-grid-3 Overview.bs:502-519,
    // 671-700) — до размера дорожек ей известен только пролёт: дорожки
    // оси решётки при замере по оси укладки `auto` по числу пролёта.
    let grid_gap_px = grid_gap_style.resolve_or_zero(
        if rows {
            inner_node_size.height
        } else {
            inner_node_size.width
        },
        |val, basis| tree.calc(val, basis),
    );
    // Позиция авто-размещённой подсетки ещё неизвестна — для замера она
    // ставится в начало контейнера, как у Blink (`grid_layout_utils.cc`
    // `AccommodateSubgridExtraMargins`: «we place them at the beginning of
    // the container for sizing»).
    for child in &children {
        let (s, e) = child.definite.unwrap_or((0, child.span.min(n).max(1)));
        let lines = Line {
            start: (2 * s) as u16,
            end: (2 * e) as u16,
        };
        // Имена родителя наследует только подсетка с ОПРЕДЕЛЁННОЙ позицией:
        // авто-размещённая в лунках их не получает (css-grid-3
        // Overview.bs:502-519).
        let names = match child.definite {
            Some(_) => name_resolver.names_in_span(
                !rows,
                s as i16 - negative_implicit as i16 + 1,
                (e - s) as u16,
            ),
            None => Vec::new(),
        };
        subgrid::publish_lanes_subgrid(
            tree,
            child.node,
            rows,
            &grid_tracks,
            lines,
            false,
            grid_gap_px,
            inner_node_size,
            names,
            flow,
        );
    }
    for child in &children {
        let mut track = GridTrack::new(
            MinTrackSizingFunction::auto(),
            MaxTrackSizingFunction::auto(),
        );
        if rows {
            let width = tree.measure_child_size(
                child.node,
                Size::NONE,
                Size {
                    width: inner_node_size.width,
                    height: None,
                },
                Size {
                    width: AvailableSpace::MaxContent,
                    height: AvailableSpace::MaxContent,
                },
                SizingMode::InherentSize,
                AbsoluteAxis::Horizontal,
                Line::FALSE,
            );
            let child_style = tree.get_grid_child_style(child.node);
            let margins = child_style.margin().map(|m| {
                m.resolve_or_zero(inner_node_size.width, |val, basis| tree.calc(val, basis))
            });
            drop(child_style);
            track.base_size = f32_max(width + margins.left + margins.right, 0.0);
            stack_max.push(Some(width));
        } else {
            stack_max.push(None);
        }
        stack_tracks.push(track);
        stack_tracks.push(GridTrack::gutter(LengthPercentage::length(0.0)));
    }
    let stack_counts = TrackCounts::from_raw(0, children.len() as u16, 0);

    // Виртуальные элементы (Overview.bs:619-669; Blink :1855-1911): явный —
    // в своих дорожках, авто — копия на каждой стартовой линии, кроме
    // пролётов через схлопнутые дорожки.
    let mut items: Vec<GridItem> = Vec::new();
    for (k, child) in children.iter().enumerate() {
        let child_style = tree.get_grid_child_style(child.node);
        let stack_line = Line {
            start: OriginZeroLine(k as i16),
            end: OriginZeroLine(k as i16 + 1),
        };
        let to_oz = |t: usize| OriginZeroLine(t as i16 - negative_implicit as i16);
        let push = |s: usize, e: usize, items: &mut Vec<GridItem>| {
            let grid_line = Line {
                start: to_oz(s),
                end: to_oz(e),
            };
            let (col, row) = if rows {
                (stack_line, grid_line)
            } else {
                (grid_line, stack_line)
            };
            items.push(GridItem::new_with_placement_style_and_order(
                child.node,
                col,
                row,
                &child_style,
                align_items.unwrap_or(AlignItems::STRETCH),
                justify_items.unwrap_or(AlignItems::STRETCH),
                child.index as u16,
            ));
        };
        match child.definite {
            Some((s, e)) => push(s, e, &mut items),
            None => {
                let span = child.span.min(n);
                for s in 0..=(n - span) {
                    if (s..s + span).any(|t| collapsed(&grid_tracks, t)) {
                        continue;
                    }
                    push(s, s + span, &mut items);
                }
            }
        }
    }
    let (col_counts, row_counts) = if rows {
        (stack_counts, counts)
    } else {
        (counts, stack_counts)
    };
    // KaminIDE patch: элементы подсеток-элементов вкладываются в дорожки оси
    // решётки — у авто-размещённой подсетки из КАЖДОЙ её виртуальной позиции
    // (css-grid-3 §track-sizing-subgrid, Overview.bs:671-700: «items of an
    // auto-placed subgrid contribute to every track they could be placed
    // in»); края подсетки — полом дорожек (Blink
    // `AccommodateSubgridExtraMargins`, ветка `is_auto_placed`).
    let grid_bit = if rows {
        crate::style::SUBGRID_ROWS
    } else {
        crate::style::SUBGRID_COLUMNS
    };
    let container_gap = if rows {
        Size {
            width: 0.0,
            height: grid_gap_px,
        }
    } else {
        Size {
            width: grid_gap_px,
            height: 0.0,
        }
    };
    let subgrid_edges = subgrid::flatten_subgrid_items(
        tree,
        &mut items,
        container_gap,
        inner_node_size,
        grid_bit,
        None,
        flow,
    );
    let auto_roots: Vec<NodeId> = children
        .iter()
        .filter(|c| c.definite.is_none())
        .map(|c| c.node)
        .collect();
    if !subgrid_edges.is_empty() {
        subgrid::apply_subgrid_floors(&mut grid_tracks, &subgrid_edges, !rows, counts);
        subgrid::apply_lanes_auto_floors(&mut grid_tracks, &subgrid_edges, !rows, |root| {
            auto_roots.contains(&root)
        });
    }
    // css-grid-3 #track-sizing-subgrid (Overview.bs:686-694): у подсетки с
    // АВТОМАТИЧЕСКОЙ позицией в лунках «Every item is placed into every
    // possible parent grid track that could be spanned by the subgrid
    // (ignoring any explicit placement of the item)», а край подсетки
    // явно размещённый элемент получает «as usual» — по своей позиции
    // (`extra_margin` уже посчитан сплющиванием). Копии виртуальных
    // позиций самой подсетки дали элементу лишь её пролёты; оставляем
    // одну копию и ставим её на каждую стартовую линию оси решётки
    // (`gap/column-subgrid-grid-gap-009`: элемент второй колонки подсетки
    // вкладывает 100 + 50 и в пустую первую `auto` — эталон 150px).
    if auto_roots
        .iter()
        .any(|r| items.iter().any(|it| it.subgrid_root == Some(*r)))
    {
        let grid_line = |it: &GridItem| if rows { it.row } else { it.column };
        let mut seen: Vec<NodeId> = Vec::new();
        let mut spread: Vec<GridItem> = Vec::new();
        items.retain(|it| {
            if it.subgrid_root.map_or(true, |r| !auto_roots.contains(&r)) {
                return true;
            }
            if !seen.contains(&it.node) {
                seen.push(it.node);
                let span = (grid_line(it).span().max(1) as usize).min(n);
                for s in 0..=(n - span) {
                    if (s..s + span).any(|t| collapsed(&grid_tracks, t)) {
                        continue;
                    }
                    let mut copy = it.clone();
                    let line = Line {
                        start: OriginZeroLine(s as i16 - negative_implicit as i16),
                        end: OriginZeroLine((s + span) as i16 - negative_implicit as i16),
                    };
                    if rows {
                        copy.row = line;
                    } else {
                        copy.column = line;
                    }
                    spread.push(copy);
                }
            }
            false
        });
        items.extend(spread);
    }
    resolve_item_track_indexes(&mut items, col_counts, row_counts);
    if rows {
        determine_if_item_crosses_flexible_or_intrinsic_tracks(
            &mut items,
            &stack_tracks,
            &grid_tracks,
        );
    } else {
        determine_if_item_crosses_flexible_or_intrinsic_tracks(
            &mut items,
            &grid_tracks,
            &stack_tracks,
        );
    }

    // Размер дорожек оси решётки — ОБЩИМ алгоритмом сетки.
    let grid_alignment =
        if rows { align_content } else { justify_content }.unwrap_or(AlignContent::STRETCH);
    let sizing_count = subgrid::partition_for_axis(&mut items, grid_axis);
    // Track sizing consumes content-box constraints; own edges are added only after sizing.
    let inner_min_size = min_size.maybe_sub(content_box_inset.sum_axes());
    let inner_max_size = max_size.maybe_sub(content_box_inset.sum_axes());
    if rows {
        track_sizing_algorithm(
            tree,
            AbstractAxis::Block,
            inner_min_size.get(AbstractAxis::Block),
            inner_max_size.get(AbstractAxis::Block),
            grid_alignment,
            AlignContent::START,
            available_grid_space,
            inner_node_size,
            &mut grid_tracks,
            &mut stack_tracks,
            &mut items[..sizing_count],
            |track: &GridTrack, _, _| Some(track.base_size),
            true,
        );
    } else {
        track_sizing_algorithm(
            tree,
            AbstractAxis::Inline,
            inner_min_size.get(AbstractAxis::Inline),
            inner_max_size.get(AbstractAxis::Inline),
            grid_alignment,
            AlignContent::START,
            available_grid_space,
            inner_node_size,
            &mut grid_tracks,
            &mut stack_tracks,
            &mut items[..sizing_count],
            |_: &GridTrack, _, _| None,
            false,
        );
    }
    drop(items);

    let grid_sum: f32 = grid_tracks.iter().map(|t| t.base_size).sum();
    inner_node_size.set(grid_axis, inner_node_size.get(grid_axis).or(Some(grid_sum)));

    // Размер по оси решётки.
    let resolved_style_size = known_dimensions.or(preferred_size);
    let grid_inset = if rows {
        content_box_inset.vertical_axis_sum()
    } else {
        content_box_inset.horizontal_axis_sum()
    };
    let (grid_min, grid_max) = (min_size.get(grid_axis), max_size.get(grid_axis));
    let grid_pb = padding_border_size.get(grid_axis);
    let grid_border_box = resolved_style_size
        .get(grid_axis)
        .unwrap_or(grid_sum + grid_inset)
        .maybe_clamp(grid_min, grid_max)
        .max(grid_pb);
    let grid_content = f32_max(0.0, grid_border_box - grid_inset);

    // Доли дорожек при неопределённом размере — от получившейся области
    // (как шаг 7 `compute_grid_layout`).
    if !available_grid_space.get(grid_axis).is_definite() {
        for track in grid_tracks.iter_mut() {
            let min: Option<f32> = track
                .min_track_sizing_function
                .resolved_percentage_size(grid_content, |val, basis| tree.calc(val, basis));
            let max: Option<f32> = track
                .max_track_sizing_function
                .resolved_percentage_size(grid_content, |val, basis| tree.calc(val, basis));
            track.base_size = track.base_size.maybe_clamp(min, max);
        }
    }

    // Выравнивание дорожек по оси решётки — как у сетки (Overview.bs:1118-1124).
    // Приставка `safe` (css-align-3 §4.4): при переполнении — к началу
    // (`align_tracks` её не знает; `grid-lanes-justify-content-001`, класс
    // `.safe`: 86 точек дорожек в коробке шириной 10).
    let grid_safe = if rows {
        align_content_safe
    } else {
        justify_content_safe
    };
    let grid_used: f32 = grid_tracks.iter().map(|t| t.base_size).sum();
    let grid_alignment = if grid_safe && grid_used > grid_content + 0.001 {
        AlignContent::START
    } else {
        grid_alignment
    };
    if rows {
        align_tracks(
            grid_content,
            Line {
                start: padding.top,
                end: padding.bottom,
            },
            Line {
                start: border.top,
                end: border.bottom,
            },
            &mut grid_tracks,
            grid_alignment,
            flow.height,
        );
    } else {
        align_tracks(
            grid_content,
            Line {
                start: padding.left,
                end: padding.right,
            },
            Line {
                start: border.left,
                end: border.right,
            },
            &mut grid_tracks,
            grid_alignment,
            flow.width,
        );
    }

    // Размещение (Overview.bs:885-968).
    let stack_avail = if rows {
        inner_node_size.width
    } else {
        inner_node_size.height
    };
    let stack_start_inset = if rows {
        content_box_inset.left
    } else {
        content_box_inset.top
    };
    let mut running: Vec<f32> = (0..n)
        .map(|t| {
            if collapsed(&grid_tracks, t) {
                f32::INFINITY
            } else {
                0.0
            }
        })
        .collect();
    let mut openings: Vec<Vec<Opening>> = (0..n)
        .map(|_| {
            vec![Opening {
                start: 0.0,
                end: f32::INFINITY,
                candidate: None,
            }]
        })
        .collect();
    let mut first_in_track: Vec<Option<usize>> = (0..n).map(|_| None).collect();
    let mut cursor: usize = if lanes.track_reverse { n } else { 0 };
    let mut placed: Vec<Placed> = Vec::with_capacity(children.len());
    let container_align = InBothAbsAxis {
        horizontal: justify_items,
        vertical: align_items,
    };
    for (k, child) in children.iter().enumerate() {
        let span = child.span.min(n).max(1);
        let (s, e) = match child.definite {
            Some(range) => range,
            None => {
                let max_of = |s: usize| {
                    (s..s + span)
                        .map(|t| running[t])
                        .fold(f32::NEG_INFINITY, f32_max)
                };
                let candidates: Vec<(usize, f32)> = (0..=(n - span))
                    .map(|s| (s, max_of(s)))
                    .filter(|(_, m)| m.is_finite())
                    .collect();
                let best = candidates
                    .iter()
                    .map(|(_, m)| *m)
                    .fold(f32::INFINITY, f32::min);
                let allowed = |m: f32| m <= best + tolerance;
                let chosen = if candidates.is_empty() {
                    0
                } else if lanes.track_reverse {
                    // Перебор назад от курсора с заворотом (Blink
                    // `RunningPositionsIterator`, :54-72).
                    let max_index = n - span;
                    let begin = if cursor < span {
                        max_index
                    } else {
                        cursor - span
                    };
                    let mut pick = None;
                    let mut i = begin;
                    for _ in 0..=max_index {
                        if let Some((_, m)) = candidates.iter().find(|(c, _)| *c == i) {
                            if allowed(*m) {
                                pick = Some(i);
                                break;
                            }
                        }
                        i = if i == 0 { max_index } else { i - 1 };
                    }
                    pick.unwrap_or(candidates[0].0)
                } else {
                    candidates
                        .iter()
                        .find(|(c, m)| *c >= cursor && allowed(*m))
                        .or_else(|| candidates.iter().find(|(_, m)| allowed(*m)))
                        .map(|(c, _)| *c)
                        .unwrap_or(candidates[0].0)
                };
                (chosen, chosen + span)
            }
        };
        let max_pos = (s..e)
            .map(|t| running[t])
            .filter(|v| v.is_finite())
            .fold(0.0f32, f32_max);
        let grid_reverse = if rows { flow.height } else { flow.width };
        let Line { start: area_start, end: area_end } = track_area(&grid_tracks, s, e, grid_reverse);
        let area = f32_max(area_end - area_start, 0.0);
        // KaminIDE patch: подсетке — размеры дорожек её пролёта (§9 (a)).
        let lines = Line {
            start: (2 * s) as u16,
            end: (2 * e) as u16,
        };
        let names = match child.definite {
            Some(_) => name_resolver.names_in_span(
                !rows,
                s as i16 - negative_implicit as i16 + 1,
                (e - s) as u16,
            ),
            None => Vec::new(),
        };
        subgrid::publish_lanes_subgrid(
            tree,
            child.node,
            rows,
            &grid_tracks,
            lines,
            true,
            grid_gap_px,
            inner_node_size,
            names,
            flow,
        );
        // Мерка по оси укладки у подсетки шла по дорожкам НАЧАЛА контейнера
        // (позиция авто-размещённой ещё не была известна); теперь дорожки —
        // её собственные, и max-content ширина рядных лунок берётся заново
        // (`row-auto-placed-subgrid-inherited-tracks-001`: доля высоты
        // ребёнка с `aspect-ratio` считается от дорожки 100, а не 50).
        if rows && subgrid::lanes_subgridded(tree, child.node, rows) {
            stack_max[k] = Some(tree.measure_child_size(
                child.node,
                Size::NONE,
                Size {
                    width: inner_node_size.width,
                    height: None,
                },
                Size {
                    width: AvailableSpace::MaxContent,
                    height: AvailableSpace::MaxContent,
                },
                SizingMode::InherentSize,
                AbsoluteAxis::Horizontal,
                Line::FALSE,
            ));
        }
        let item = layout_lanes_item(
            tree,
            child.node,
            rows,
            lanes.stack_block,
            area,
            stack_avail,
            None,
            stack_max[k],
            container_align,
        );
        let (m_start, m_end) = if rows {
            (item.margin.left, item.margin.right)
        } else {
            (item.margin.top, item.margin.bottom)
        };
        let stack_size = if rows {
            item.size.width
        } else {
            item.size.height
        };
        let outer_stack = stack_size + m_start + m_end;
        let contribution = f32_max(outer_stack + stack_gap, 0.0);
        let index = placed.len();
        // Плотная укладка (Overview.bs:871-883, 945-960; Blink
        // `GetEligibleTrackOpeningAndUpdateGridLanesItemSpan`,
        // `grid_lanes_running_positions.cc:483-689`): элемент, уже размеренный
        // в обычной позиции, уходит в более ранний пропуск, если там те же
        // суммарные размеры дорожек; бегущие позиции и курсор не меняются.
        if lanes.dense {
            let sizes: Vec<f32> = (0..n).map(|t| grid_tracks[2 * t + 1].base_size).collect();
            let used: f32 = sizes[s..e].iter().sum();
            let starts: Vec<usize> = match child.definite {
                Some(_) => vec![s],
                None if lanes.track_reverse => (0..=(n - span)).rev().collect(),
                None => (0..=(n - span)).collect(),
            };
            let mut found: Vec<(usize, f32, Vec<usize>)> = Vec::new();
            for &start in &starts {
                if (start..start + span).any(|t| collapsed(&grid_tracks, t)) {
                    continue;
                }
                let total: f32 = sizes[start..start + span].iter().sum();
                if (total - used).abs() > 0.01 {
                    continue;
                }
                if let Some((pos, mut path)) =
                    dense_path(&openings, start, span - 1, 0.0, f32::INFINITY, contribution)
                {
                    path.reverse();
                    found.push((start, pos, path));
                }
            }
            let lowest = found.iter().map(|f| f.1).fold(f32::INFINITY, f32::min);
            // Среди пропусков в пределах порога от самого высокого берётся
            // самый РАННИЙ по порядку дорожек (Overview.bs:945-952: «place it
            // in the start-most of them»); если это и есть обычная позиция —
            // элемент остаётся на месте. Отбор «раньше обычной» ДО выбора
            // уводил элемент мимо обычной лунки в соседний хвост
            // (`column-dense-packing-flow-tolerance-001`: Item 4a обязан встать
            // под Item 1).
            let chosen = found
                .into_iter()
                .find(|(_, pos, _)| *pos <= lowest + tolerance)
                .filter(|(start, pos, _)| {
                    *pos < max_pos - 0.001
                        || ((*pos - max_pos).abs() <= 0.001
                            && if lanes.track_reverse {
                                *start > s
                            } else {
                                *start < s
                            })
                });
            if let Some((start, pos, path)) = chosen {
                let end = pos + contribution;
                for (offset, &i) in path.iter().enumerate() {
                    let track = &mut openings[start + offset];
                    let o = track[i];
                    let above = o.start < pos;
                    let below = end < o.end;
                    match (above, below) {
                        (true, true) => {
                            track.insert(
                                i,
                                Opening {
                                    start: o.start,
                                    end: pos,
                                    candidate: o.candidate,
                                },
                            );
                            track[i + 1].start = end;
                            track[i + 1].candidate = Some(index);
                        }
                        (true, false) => track[i].end = pos,
                        (false, false) => {
                            track.remove(i);
                        }
                        (false, true) => {
                            track[i].start = end;
                            track[i].candidate = Some(index);
                        }
                    }
                }
                // KaminIDE patch: бегущая позиция дорожки — начало её
                // хвостового (бесконечного) проёма (Blink
                // grid_lanes_running_positions.h:430-432
                // `GetRunningPositionForTrack`). Пропуск, взятый плотной
                // укладкой из ХВОСТА (дорожка ещё пустая ниже), сдвигает и её:
                // прежде явный элемент той же дорожки вставал поверх
                // (`column-dense-packing-multi-span-012`: Item 7 в колонке 3
                // на месте Item 5/6).
                for t in start..start + span {
                    if running[t].is_finite() {
                        if let Some(last) = openings[t].last() {
                            running[t] = f32_max(running[t], last.start);
                        }
                    }
                }
                let area_start = track_area(&grid_tracks, start, start + span, grid_reverse).start;
                placed.push(Placed {
                    node: child.node,
                    start,
                    end: start + span,
                    grid_pos: area_start + item.grid_offset,
                    stack_pos: pos,
                    placement_stack_pos: pos,
                    size: item.size,
                    margin: item.margin,
                    stack_relative: item.stack_relative,
                    outer_stack,
                    padding: item.padding,
                    border: item.border,
                    scrollbar_size: item.scrollbar_size,
                    scrollable_overflow_rect: item.scrollable_overflow_rect,
                    contain: item.contain,
                    overflow: item.overflow,
                    baseline: item.baseline,
                    baseline_x: item.baseline_x,
                    last_baseline_x: item.last_baseline_x,
                    baseline_x_flags: item.baseline_x_flags,
                    stack_align: item.stack_align,
                    stack_size_fixed: item.stack_size_fixed,
                    area,
                    stack_fit: stack_max[k],
                    grid_baseline: item.grid_baseline,
                    last_baseline: item.last_baseline,
                    grid_last_baseline: item.grid_last_baseline,
                });
                continue;
            }
        }
        for t in s..e {
            if !running[t].is_finite() {
                continue;
            }
            let last = openings[t]
                .last_mut()
                .expect("у дорожки всегда есть хвостовой проём");
            if running[t] < max_pos {
                last.end = max_pos;
                openings[t].push(Opening {
                    start: max_pos + contribution,
                    end: f32::INFINITY,
                    candidate: Some(index),
                });
            } else {
                last.start = max_pos + contribution;
                last.candidate = Some(index);
            }
            running[t] = max_pos + contribution;
            if first_in_track[t].is_none() {
                first_in_track[t] = Some(index);
            }
        }
        cursor = if lanes.track_reverse { s } else { e };
        placed.push(Placed {
            node: child.node,
            start: s,
            end: e,
            grid_pos: area_start + item.grid_offset,
            stack_pos: max_pos,
            placement_stack_pos: max_pos,
            size: item.size,
            margin: item.margin,
            stack_relative: item.stack_relative,
            outer_stack,
            padding: item.padding,
            border: item.border,
            scrollbar_size: item.scrollbar_size,
            scrollable_overflow_rect: item.scrollable_overflow_rect,
            contain: item.contain,
            overflow: item.overflow,
            baseline: item.baseline,
                    baseline_x: item.baseline_x,
                    last_baseline_x: item.last_baseline_x,
                    baseline_x_flags: item.baseline_x_flags,
            stack_align: item.stack_align,
            stack_size_fixed: item.stack_size_fixed,
            area,
            stack_fit: stack_max[k],
            grid_baseline: item.grid_baseline,
            last_baseline: item.last_baseline,
            grid_last_baseline: item.grid_last_baseline,
        });
    }

    // Протяжённость укладки: наибольшая бегущая позиция минус хвостовой зазор
    // (Blink :718-723).
    let stacking_size = if placed.is_empty() {
        0.0
    } else {
        f32_max(
            running
                .iter()
                .copied()
                .filter(|v| v.is_finite())
                .fold(0.0f32, f32_max)
                - stack_gap,
            0.0,
        )
    };

    let stack_axis = if rows {
        AbstractAxis::Inline
    } else {
        AbstractAxis::Block
    };
    let stack_inset = if rows {
        content_box_inset.horizontal_axis_sum()
    } else {
        content_box_inset.vertical_axis_sum()
    };
    // Соотношение сторон без заданного размера по оси укладки: размер
    // выводится из оси решётки (`column-auto-repeat-003`: `aspect-ratio: 1`,
    // две дорожки по 50 → квадрат 100).
    let ratio_stack = aspect_ratio.map(|r| {
        if rows {
            grid_border_box * r
        } else {
            grid_border_box / r
        }
    });
    let stack_border_box = resolved_style_size
        .get(stack_axis)
        .or(ratio_stack)
        .unwrap_or(stacking_size + stack_inset)
        .maybe_clamp(min_size.get(stack_axis), max_size.get(stack_axis))
        .max(padding_border_size.get(stack_axis));
    let container_border_box = if rows {
        Size {
            width: stack_border_box,
            height: grid_border_box,
        }
    } else {
        Size {
            width: grid_border_box,
            height: stack_border_box,
        }
    };
    if run_mode == RunMode::ComputeSize {
        return LayoutOutput::from_outer_size(container_border_box);
    }
    let stack_content = f32_max(0.0, stack_border_box - stack_inset);
    let effective_stack = stack_avail.unwrap_or(stack_content);

    // Обратное заполнение: элементы ложатся от КОНЦА оси укладки
    // (Blink :1463-1480; при неопределённом размере — после размещения,
    // `ShouldDeferFillReverse` :429-434).
    if lanes.fill_reverse {
        for p in placed.iter_mut() {
            p.stack_pos = effective_stack - p.stack_pos - p.outer_stack;
        }
    }

    // Раздача содержимого по оси укладки — ОДИН субъект на весь контейнер
    // (Overview.bs:1126-1159; Blink `AlignContentOffset` :436-494): нормаль и
    // растяжка — начало, распределяющие значения — их запасные.
    let stack_content_alignment = if rows { justify_content } else { align_content };
    if let Some(alignment) = stack_content_alignment {
        let offset = stacking_alignment_offset(
            effective_stack - stacking_size,
            alignment,
            lanes.fill_reverse,
        );
        if offset != 0.0 {
            for p in placed.iter_mut() {
                p.stack_pos += offset;
            }
        }
    }

    // Самовыравнивание по оси укладки — только у элементов над «проёмом»
    // (Overview.bs:1161-1225; Blink `ApplyStackingAxisAlignment` :1018-1157).
    // При `fill-reverse` на месте стоят элементы с `end`, двигать надо `start`
    // (Blink :1097-1119).
    let moves = |a: Option<AlignSelf>| match a.map(|a| a.keyword) {
        Some(AlignItemsKeyword::Start | AlignItemsKeyword::FlexStart) => lanes.fill_reverse,
        Some(AlignItemsKeyword::End | AlignItemsKeyword::FlexEnd) => !lanes.fill_reverse,
        Some(AlignItemsKeyword::Stretch | AlignItemsKeyword::Center) => true,
        _ => false,
    };
    if placed.iter().any(|p| moves(p.stack_align)) {
        for track in openings.iter_mut() {
            if let Some(last) = track.last_mut() {
                last.start -= stack_gap;
                last.end = effective_stack;
            }
        }
        for index in 0..placed.len() {
            let Some(alignment) = placed[index].stack_align else {
                continue;
            };
            let (s, e) = (placed[index].start, placed[index].end);
            let mut space = f32::INFINITY;
            for track in openings.iter().take(e).skip(s) {
                match track.iter().find(|o| o.candidate == Some(index)) {
                    Some(o) => space = space.min(o.end - o.start),
                    None => {
                        space = 0.0;
                        break;
                    }
                }
            }
            if !space.is_finite() || space <= 0.0 {
                continue;
            }
            match alignment.keyword {
                AlignItemsKeyword::Stretch => {
                    if placed[index].stack_size_fixed {
                        continue;
                    }
                    let grown = (if rows {
                        placed[index].size.width
                    } else {
                        placed[index].size.height
                    }) + space;
                    let item = layout_lanes_item(
                        tree,
                        placed[index].node,
                        rows,
                        lanes.stack_block,
                        placed[index].area,
                        stack_avail,
                        Some(grown),
                        placed[index].stack_fit,
                        container_align,
                    );
                    let p = &mut placed[index];
                    p.size = item.size;
                    p.scrollable_overflow_rect = item.scrollable_overflow_rect;
                    p.contain = item.contain;
                    p.baseline = item.baseline;
                    p.last_baseline = item.last_baseline;
                    p.baseline_x = item.baseline_x;
                    p.last_baseline_x = item.last_baseline_x;
                    if lanes.fill_reverse {
                        p.stack_pos -= space;
                    }
                }
                AlignItemsKeyword::Center => {
                    placed[index].stack_pos += if lanes.fill_reverse {
                        -space / 2.0
                    } else {
                        space / 2.0
                    }
                }
                AlignItemsKeyword::End | AlignItemsKeyword::FlexEnd if !lanes.fill_reverse => {
                    placed[index].stack_pos += space
                }
                AlignItemsKeyword::Start | AlignItemsKeyword::FlexStart if lanes.fill_reverse => {
                    placed[index].stack_pos -= space
                }
                _ => {}
            }
        }
    }

    baseline_alignment::align(&mut placed, n, rows);

    #[cfg_attr(not(feature = "content_size"), allow(unused_mut))]
    let mut item_content_size_contribution = final_placement::position(
        tree, &mut placed, rows, flow, stack_content, stack_start_inset, border,
        #[cfg(feature = "content_size")] is_scroll_container,
    );
    // The stacking axis exports the highest first and lowest last baseline
    // across tracks (css-grid-3 §6.5), using actual content baseline sets.
    let baselines = container_baselines::compute(&placed, &first_in_track, rows, stack_start_inset);

    // Скрытые и абсолютные дети. По оси укладки линий две — края
    // протяжённости укладки (Overview.bs:1303-1315); здесь — край
    // контейнера, как `auto`.
    let mut order = placed.len() as u32;
    let child_count = tree.child_count(node);
    for index in 0..child_count {
        let child = tree.get_child_id(node, index);
        let child_style = tree.get_grid_child_style(child);
        if child_style.box_generation_mode() == BoxGenerationMode::None {
            drop(child_style);
            tree.set_unrounded_layout(child, &Layout::with_order(order));
            tree.perform_child_layout(
                child,
                Size::NONE,
                Size::NONE,
                Size::MAX_CONTENT,
                SizingMode::InherentSize,
                Line::FALSE,
            );
            order += 1;
            continue;
        }
        if child_style.position() != Position::Absolute {
            continue;
        }
        let placement = if rows {
            name_resolver.resolve_row_names(&child_style.grid_row())
        } else {
            name_resolver.resolve_column_names(&child_style.grid_column())
        }
        .into_origin_zero(explicit)
        .resolve_absolutely_positioned_grid_tracks()
        .map(|line| line.and_then(|line: OriginZeroLine| line.try_into_track_vec_index(counts)));
        drop(child_style);
        let (grid_lo, grid_hi) = if rows {
            (
                border.top,
                container_border_box.height - border.bottom - scrollbar_gutter.y,
            )
        } else {
            (
                border.left,
                container_border_box.width - border.right - scrollbar_gutter.x,
            )
        };
        let Line { start: lo, end: hi } = absolute_area(&grid_tracks, placement, grid_lo, grid_hi,
            if rows { flow.height } else { flow.width });
        let grid_area = if rows {
            Rect {
                top: lo,
                bottom: hi,
                left: border.left,
                right: container_border_box.width - border.right - scrollbar_gutter.x,
            }
        } else {
            Rect {
                left: lo,
                right: hi,
                top: border.top,
                bottom: container_border_box.height - border.bottom - scrollbar_gutter.y,
            }
        };
        #[cfg_attr(not(feature = "content_size"), allow(unused_variables))]
        let (content_size_contribution, _, _, _, _) = super::alignment::align_and_position_item(
            tree,
            child,
            order,
            grid_area,
            container_align,
            Rect::ZERO,
            false,
            0,
            if flow.width { Direction::Rtl } else { Direction::Ltr },
            container_border_box.width,
            border,
            #[cfg(feature = "content_size")]
            is_scroll_container,
        );
        #[cfg(feature = "content_size")]
        {
            item_content_size_contribution =
                item_content_size_contribution.union(content_size_contribution);
        }
        order += 1;
    }

    LayoutOutput::from_sizes_and_baselines(
        container_border_box,
        item_content_size_contribution,
        if contain.intersects(Contain::LAYOUT) {
            Baselines::NONE
        } else {
            baselines
        },
    )
}

/// Шаблон оси решётки — своими данными (стиль занимает дерево, а
/// гипотетический размер требует его на изменение). `Some` — только когда в
/// шаблоне ровно один `repeat(auto-*)` и в нём есть дорожка без
/// фиксированной грани (иначе счёт общий, `compute_explicit_grid_size_in_axis`).
enum OwnedComponent {
    Single(TrackSizingFunction),
    Count(u16, Vec<TrackSizingFunction>),
    Auto(Vec<TrackSizingFunction>),
}

fn owned_template(
    style: &impl GridContainerStyle,
    axis: AbsoluteAxis,
) -> Option<Vec<OwnedComponent>> {
    let template = match axis {
        AbsoluteAxis::Horizontal => style.grid_template_columns(),
        AbsoluteAxis::Vertical => style.grid_template_rows(),
    }?;
    let mut out = Vec::new();
    let mut autos = 0;
    let mut intrinsic = false;
    for component in template {
        match component {
            GenericGridTemplateComponent::Single(f) => out.push(OwnedComponent::Single(f)),
            GenericGridTemplateComponent::Repeat(r) => {
                let tracks: Vec<TrackSizingFunction> = r.tracks().collect();
                if tracks.is_empty() {
                    return None;
                }
                match r.count() {
                    RepetitionCount::Count(c) => out.push(OwnedComponent::Count(c, tracks)),
                    RepetitionCount::AutoFill | RepetitionCount::AutoFit => {
                        autos += 1;
                        intrinsic |= tracks.iter().any(|t| !t.has_fixed_component());
                        out.push(OwnedComponent::Auto(tracks));
                    }
                }
            }
        }
    }
    (autos == 1 && intrinsic).then_some(out)
}

/// Число повторов интрин-`repeat(auto-*)` по гипотетическим размерам
/// (css-grid-3 §7.2.1, Overview.bs:444-500; Blink
/// `GridLanesLayoutAlgorithm::ComputeAutomaticRepetitions`, :2999): явное
/// размещение не учитывается, ничего не схлопывается, тело повторяется
/// `2 + (наибольший пролёт − 2) / (дорожек в теле)` раз (вниз, не меньше
/// двух); размер записи тела — наибольший среди её копий. Возвращает число
/// повторов и число дорожек явной сетки.
#[allow(clippy::too_many_arguments)]
fn intrinsic_repetitions<Tree: LayoutGridContainer>(
    tree: &mut Tree,
    node: NodeId,
    template: &[OwnedComponent],
    gap_style: LengthPercentage,
    rows: bool,
    avail: Option<f32>,
    strategy: AutoRepeatStrategy,
    inner_node_size: Size<Option<f32>>,
    align_items: Option<AlignItems>,
    justify_items: Option<AlignItems>,
) -> (u16, u16) {
    let mut spans: Vec<(NodeId, usize, u16)> = Vec::new();
    for (index, child) in tree.child_ids(node).enumerate() {
        let style = tree.get_grid_child_style(child);
        if style.box_generation_mode() == BoxGenerationMode::None
            || style.position() == Position::Absolute
        {
            continue;
        }
        let line = if rows {
            style.grid_row()
        } else {
            style.grid_column()
        };
        let span = match (&line.start, &line.end) {
            (GridPlacement::Line(a), GridPlacement::Line(b)) => {
                (b.as_i16() - a.as_i16()).unsigned_abs().max(1)
            }
            (GridPlacement::Span(k), _) | (_, GridPlacement::Span(k)) => (*k).max(1),
            _ => 1,
        };
        spans.push((child, index, span));
    }
    let body_len = template
        .iter()
        .find_map(|c| match c {
            OwnedComponent::Auto(t) => Some(t.len()),
            _ => None,
        })
        .unwrap_or(1);
    let largest = spans.iter().map(|s| s.2 as usize).max().unwrap_or(1);
    let copies = (2 + largest.saturating_sub(2) / body_len).max(2);
    // Развёрнутый шаблон: функция дорожки и номер записи тела повтора.
    let mut expanded: Vec<(TrackSizingFunction, Option<usize>)> = Vec::new();
    let mut non_repeat = 0u16;
    for c in template {
        match c {
            OwnedComponent::Single(f) => {
                expanded.push((*f, None));
                non_repeat += 1;
            }
            OwnedComponent::Count(n, t) => {
                for _ in 0..*n {
                    expanded.extend(t.iter().map(|f| (*f, None)));
                }
                non_repeat += n * t.len() as u16;
            }
            OwnedComponent::Auto(t) => {
                for _ in 0..copies {
                    expanded.extend(t.iter().enumerate().map(|(j, f)| (*f, Some(j))));
                }
            }
        }
    }
    let n = expanded.len();
    let mut grid_tracks: GridTrackVec<GridTrack> = GridTrackVec::new();
    grid_tracks.push(GridTrack::gutter(LengthPercentage::length(0.0)));
    for (i, (f, _)) in expanded.iter().enumerate() {
        grid_tracks.push(GridTrack::new(
            f.min_sizing_function(),
            f.max_sizing_function(),
        ));
        // Крайние зазоры нулевые, внутренние — зазор сетки (как у
        // `initialize_grid_tracks`).
        let gutter = if i + 1 < n {
            gap_style
        } else {
            LengthPercentage::length(0.0)
        };
        grid_tracks.push(GridTrack::gutter(gutter));
    }
    let mut stack_tracks: GridTrackVec<GridTrack> = GridTrackVec::new();
    stack_tracks.push(GridTrack::gutter(LengthPercentage::length(0.0)));
    for (child, _, _) in &spans {
        let mut track = GridTrack::new(
            MinTrackSizingFunction::auto(),
            MaxTrackSizingFunction::auto(),
        );
        if rows {
            let width = tree.measure_child_size(
                *child,
                Size::NONE,
                Size {
                    width: inner_node_size.width,
                    height: None,
                },
                Size::MAX_CONTENT,
                SizingMode::InherentSize,
                AbsoluteAxis::Horizontal,
                Line::FALSE,
            );
            track.base_size = width;
        }
        stack_tracks.push(track);
        stack_tracks.push(GridTrack::gutter(LengthPercentage::length(0.0)));
    }
    let mut items: Vec<GridItem> = Vec::new();
    for (k, (child, index, span)) in spans.iter().enumerate() {
        let style = tree.get_grid_child_style(*child);
        let span = (*span as usize).min(n);
        let stack_line = Line {
            start: OriginZeroLine(k as i16),
            end: OriginZeroLine(k as i16 + 1),
        };
        for s in 0..=(n - span) {
            let grid_line = Line {
                start: OriginZeroLine(s as i16),
                end: OriginZeroLine((s + span) as i16),
            };
            let (col, row) = if rows {
                (stack_line, grid_line)
            } else {
                (grid_line, stack_line)
            };
            items.push(GridItem::new_with_placement_style_and_order(
                *child,
                col,
                row,
                &style,
                align_items.unwrap_or(AlignItems::STRETCH),
                justify_items.unwrap_or(AlignItems::STRETCH),
                *index as u16,
            ));
        }
    }
    let grid_counts = TrackCounts::from_raw(0, n as u16, 0);
    let stack_counts = TrackCounts::from_raw(0, spans.len() as u16, 0);
    let axis = if rows {
        AbstractAxis::Block
    } else {
        AbstractAxis::Inline
    };
    let mut inner = inner_node_size;
    inner.set(axis, None);
    let mut space = Size::MAX_CONTENT;
    if rows {
        space.width = inner_node_size
            .width
            .map(AvailableSpace::Definite)
            .unwrap_or(AvailableSpace::MaxContent);
    } else {
        space.height = inner_node_size
            .height
            .map(AvailableSpace::Definite)
            .unwrap_or(AvailableSpace::MaxContent);
    }
    if rows {
        resolve_item_track_indexes(&mut items, stack_counts, grid_counts);
        determine_if_item_crosses_flexible_or_intrinsic_tracks(
            &mut items,
            &stack_tracks,
            &grid_tracks,
        );
        track_sizing_algorithm(
            tree,
            AbstractAxis::Block,
            None,
            None,
            AlignContent::START,
            AlignContent::START,
            space,
            inner,
            &mut grid_tracks,
            &mut stack_tracks,
            &mut items,
            |track: &GridTrack, _, _| Some(track.base_size),
            false,
        );
    } else {
        resolve_item_track_indexes(&mut items, grid_counts, stack_counts);
        determine_if_item_crosses_flexible_or_intrinsic_tracks(
            &mut items,
            &grid_tracks,
            &stack_tracks,
        );
        track_sizing_algorithm(
            tree,
            AbstractAxis::Inline,
            None,
            None,
            AlignContent::START,
            AlignContent::START,
            space,
            inner,
            &mut grid_tracks,
            &mut stack_tracks,
            &mut items,
            |_: &GridTrack, _, _| None,
            false,
        );
    }
    // Гипотетический размер — предел роста после шага §12.5 (Blink
    // :1880-1890: «we need to use the growth limit as the track size»).
    let size_of = |i: usize| {
        let t = &grid_tracks[2 * i + 1];
        if t.growth_limit.is_finite() {
            f32_max(t.base_size, t.growth_limit)
        } else {
            t.base_size
        }
    };
    let mut body = Vec::with_capacity(body_len);
    body.resize(body_len, 0.0f32);
    let mut fixed = 0.0f32;
    for (i, (_, j)) in expanded.iter().enumerate() {
        match j {
            // Пол 1px (css-grid-2 §7.2.3.2, Overview.bs:1944: «It is suggested
            // that this floor be 1px»): пустые элементы иначе дают нулевой шаг,
            // и повтор оставался одним (`column-auto-repeat-auto-028`).
            Some(j) => body[*j] = f32_max(body[*j], f32_max(size_of(i), 1.0)),
            None => fixed += size_of(i),
        }
    }
    let Some(room) = avail else {
        return (1, non_repeat + body_len as u16);
    };
    let gap = gap_style.resolve_or_zero(Some(room), |val, basis| tree.calc(val, basis));
    let body_sum: f32 = body.iter().sum();
    let per_rep = body_sum + gap * body_len as f32;
    let first = fixed + body_sum + gap * (non_repeat as usize + body_len).saturating_sub(1) as f32;
    let reps = if first > room || per_rep <= 0.0 {
        1u16
    } else {
        let fit = (room - first) / per_rep;
        let extra = match strategy {
            AutoRepeatStrategy::MaxRepetitionsThatDoNotOverflow => fit.floor(),
            AutoRepeatStrategy::MinRepetitionsThatDoOverflow => fit.ceil(),
        };
        (extra.clamp(0.0, 1000.0) as u16) + 1
    };
    (reps, non_repeat + reps * body_len as u16)
}

/// Путь проёмов для плотной укладки: в дорожке `t` и следующих `remaining`
/// ищется цепочка проёмов, общий кусок которых вмещает `need` (Blink
/// `AccumulateTrackOpeningsToAccommodateItem`, :374-434). Возвращает начало
/// общего куска и индексы проёмов от ПОСЛЕДНЕЙ дорожки к первой.
fn dense_path(
    openings: &[Vec<Opening>],
    t: usize,
    remaining: usize,
    lo: f32,
    hi: f32,
    need: f32,
) -> Option<(f32, Vec<usize>)> {
    for (i, o) in openings[t].iter().enumerate() {
        let start = f32_max(lo, o.start);
        let end = hi.min(o.end);
        if start > end || end - start < need {
            continue;
        }
        if remaining == 0 {
            return Some((start, vec![i]));
        }
        if let Some((pos, mut path)) = dense_path(openings, t + 1, remaining - 1, start, end, need)
        {
            path.push(i);
            return Some((pos, path));
        }
    }
    None
}

/// Раскладка одного элемента лунок: область по оси решётки известна, по
/// оси укладки размер — по содержимому (Overview.bs:970-975: содержащий
/// блок — область по оси решётки и область содержимого контейнера по оси
/// укладки). `stack_known` — размер по оси укладки при растяжке.
#[allow(clippy::too_many_arguments)]
fn layout_lanes_item(
    tree: &mut impl LayoutGridContainer,
    node: NodeId,
    rows: bool,
    stack_block: bool,
    area: f32,
    stack_avail: Option<f32>,
    stack_known: Option<f32>,
    stack_fit: Option<f32>,
    container_align: InBothAbsAxis<Option<AlignItems>>,
) -> ItemBox {
    let style = tree.get_grid_child_style(node);
    // Содержащий блок: по оси решётки — область, по оси укладки — область
    // содержимого контейнера.
    let cb = if rows {
        Size {
            width: stack_avail,
            height: Some(area),
        }
    } else {
        Size {
            width: Some(area),
            height: stack_avail,
        }
    };
    let overflow = style.overflow();
    let scrollbar_width = style.scrollbar_width();
    let contain = style.contain();
    let aspect_ratio = style.aspect_ratio();
    let baseline_x_flags = style.baseline_x_flags();
    let item_direction = style.direction();
    let justify_self = style
        .justify_self()
        .map(|a| a.resolve_self_relative(item_direction, Direction::Ltr, true));
    let align_self = style
        .align_self()
        .map(|a| a.resolve_self_relative(item_direction, Direction::Ltr, false));
    let position = style.position();
    let inset_h = style
        .inset()
        .horizontal_components()
        .map(|v| v.resolve_to_option(cb.width.unwrap_or(0.0), |val, basis| tree.calc(val, basis)));
    let inset_v = style
        .inset()
        .vertical_components()
        .map(|v| v.resolve_to_option(cb.height.unwrap_or(0.0), |val, basis| tree.calc(val, basis)));
    let padding = style
        .padding()
        .map(|p| p.resolve_or_zero(cb.width, |val, basis| tree.calc(val, basis)));
    let border = style
        .border()
        .map(|p| p.resolve_or_zero(cb.width, |val, basis| tree.calc(val, basis)));
    let padding_border_size = (padding + border).sum_axes();
    let box_sizing_adjustment = if style.box_sizing() == BoxSizing::ContentBox {
        padding_border_size
    } else {
        Size::ZERO
    };
    let raw_margin = style.margin();
    let mut size_style = style.size();
    let style_size = size_style.maybe_resolve(cb, |val, basis| tree.calc(val, basis));
    let inherent_size = style_size
        .maybe_apply_aspect_ratio(aspect_ratio)
        .maybe_add(box_sizing_adjustment);
    let min_size = style
        .min_size()
        .maybe_resolve(cb, |val, basis| tree.calc(val, basis))
        .maybe_add(box_sizing_adjustment)
        .or(padding_border_size.map(Some))
        .maybe_max(padding_border_size)
        .maybe_apply_aspect_ratio(aspect_ratio);
    let max_size = style
        .max_size()
        .maybe_resolve(cb, |val, basis| tree.calc(val, basis))
        .maybe_apply_aspect_ratio(aspect_ratio)
        .maybe_add(box_sizing_adjustment);
    let mut margin = raw_margin
        .map(|m| m.resolve_to_option(cb.width.unwrap_or(0.0), |val, basis| tree.calc(val, basis)));
    drop(style);
    // KaminIDE patch: подсетка всегда растянута в подсеточной оси, её
    // выравнивание и размеры там игнорируются (css-grid-2
    // §subgrid-box-alignment) — иначе линии подсетки не совпали бы с
    // дорожками, которые ей выданы (`subgrid::publish_lanes_subgrid`).
    let subgridded = position != Position::Absolute && subgrid::lanes_subgridded(tree, node, rows);
    let (mut inherent_size, mut min_size, mut max_size) = (inherent_size, min_size, max_size);
    let (justify_self, align_self) = if subgridded {
        if rows {
            inherent_size.height = None;
            size_style.height = crate::Dimension::auto();
            min_size.height = Some(padding_border_size.height);
            max_size.height = None;
            margin.top = margin.top.or(Some(0.0));
            margin.bottom = margin.bottom.or(Some(0.0));
            (justify_self, Some(AlignSelf::STRETCH))
        } else {
            inherent_size.width = None;
            size_style.width = crate::Dimension::auto();
            min_size.width = Some(padding_border_size.width);
            max_size.width = None;
            margin.left = margin.left.or(Some(0.0));
            margin.right = margin.right.or(Some(0.0));
            (Some(AlignSelf::STRETCH), align_self)
        }
    } else {
        (justify_self, align_self)
    };

    // Выравнивание по оси решётки — как у сетки: `normal` растягивает, если
    // размер не задан.
    let grid_self = if rows {
        align_self
            .or(container_align.vertical)
            .map(|a| a.resolve_self_relative(item_direction, Direction::Ltr, false))
    } else {
        justify_self
            .or(container_align.horizontal)
            .map(|a| a.resolve_self_relative(item_direction, Direction::Ltr, true))
    };
    let stack_align = if rows {
        justify_self
            .or(container_align.horizontal)
            .map(|a| a.resolve_self_relative(item_direction, Direction::Ltr, true))
    } else {
        align_self
            .or(container_align.vertical)
            .map(|a| a.resolve_self_relative(item_direction, Direction::Ltr, false))
    };
    // Resolve explicit sizing keywords before normal/stretch alignment; linked axes
    // were masked to auto above, so their authored keywords cannot trigger probes.
    let keyword_stretch_size = Size {
        width: cb
            .width
            .map(|v| (v - margin.left.unwrap_or(0.0) - margin.right.unwrap_or(0.0)).max(0.0)),
        height: cb
            .height
            .map(|v| (v - margin.top.unwrap_or(0.0) - margin.bottom.unwrap_or(0.0)).max(0.0)),
    };
    let keyword_available = Size {
        width: keyword_stretch_size
            .width
            .map(AvailableSpace::Definite)
            .unwrap_or(AvailableSpace::MaxContent),
        height: if !rows && stack_block {
            AvailableSpace::MaxContent
        } else {
            keyword_stretch_size
                .height
                .map(AvailableSpace::Definite)
                .unwrap_or(AvailableSpace::MaxContent)
        },
    };
    let keyword_size = super::lanes_sizing::resolve_lane_sizing_keywords(
        tree,
        node,
        size_style,
        cb,
        keyword_stretch_size,
        keyword_available,
        inherent_size,
    );
    inherent_size = inherent_size.or(keyword_size);
    let inherent_grid = if rows {
        inherent_size.height
    } else {
        inherent_size.width
    };
    let grid_alignment =
        grid_self.unwrap_or(if inherent_grid.is_some() || aspect_ratio.is_some() {
            AlignSelf::START
        } else {
            AlignSelf::STRETCH
        });
    let (gm_start, gm_end) = if rows {
        (margin.top, margin.bottom)
    } else {
        (margin.left, margin.right)
    };
    let grid_known = inherent_grid.or_else(|| {
        (grid_alignment.keyword == AlignItemsKeyword::Stretch
            && gm_start.is_some()
            && gm_end.is_some()
            && position != Position::Absolute)
            .then(|| f32_max(area - gm_start.unwrap_or(0.0) - gm_end.unwrap_or(0.0), 0.0))
    });
    let stack_margin = if rows {
        margin.left.unwrap_or(0.0) + margin.right.unwrap_or(0.0)
    } else {
        margin.top.unwrap_or(0.0) + margin.bottom.unwrap_or(0.0)
    };
    let inherent_stack = if rows {
        inherent_size.width
    } else {
        inherent_size.height
    };
    let stack_size_fixed = inherent_stack.is_some();
    // Без своего размера по оси укладки элемент меряется по содержимому:
    // max-content, но не шире области содержимого контейнера (fit-content).
    // KaminIDE patch: ось укладки — БЛОЧНАЯ ось контейнера (вертикальное
    // письмо, лунки колонками по логике — физически рядами): блочный размер
    // элемента — по его содержимому при строчном размере области, места
    // контейнера по этой оси он не знает (css-grid-3 Overview.bs:1107-1116,
    // как ось укладки колоночных лунок горизонтального письма).
    let stack_fit = if stack_block {
        None
    } else if rows && inherent_stack.is_none() && stack_known.is_none() {
        // Early max-content width predates the resolved track height. Vertical
        // text may now wrap into more columns, which changes stacking width.
        Some(super::lanes_stack_measure::width(tree, node, grid_known, cb, area))
    } else {
        stack_fit
    };
    let fit = stack_fit.map(|w| match stack_avail {
        Some(avail) => w.min(f32_max(avail - stack_margin, 0.0)),
        None => w,
    });
    let stack_known_size = inherent_stack
        .or(stack_known.map(|v| f32_max(v, 0.0)))
        .or(fit);
    let known = if rows {
        Size {
            width: stack_known_size,
            height: grid_known,
        }
    } else {
        Size {
            width: grid_known,
            height: stack_known_size,
        }
    };
    let known = known
        .maybe_apply_aspect_ratio(aspect_ratio)
        .maybe_clamp(min_size, max_size);
    let grid_avail = AvailableSpace::Definite(f32_max(
        area - gm_start.unwrap_or(0.0) - gm_end.unwrap_or(0.0),
        0.0,
    ));
    let stack_space = match stack_avail {
        Some(v) if !stack_block => AvailableSpace::Definite(f32_max(v - stack_margin, 0.0)),
        _ => AvailableSpace::MaxContent,
    };
    let available = if rows {
        Size {
            width: stack_space,
            height: grid_avail,
        }
    } else {
        Size {
            width: grid_avail,
            height: stack_space,
        }
    };
    let output = tree.perform_child_layout(
        node,
        known,
        cb,
        available,
        SizingMode::InherentSize,
        Line::FALSE,
    );
    let size = known.unwrap_or(output.size).maybe_clamp(min_size, max_size);
    let grid_item_size = if rows { size.height } else { size.width };
    let grid_position_alignment = if rows {
        grid_alignment
    } else {
        super::baseline_orientation::projected_x_alignment(grid_alignment, baseline_x_flags & 1 != 0, Direction::Ltr)
    };
    let (grid_offset, grid_margin) = align_item_within_area(
        Line {
            start: 0.0,
            end: area,
        },
        grid_position_alignment,
        grid_item_size,
        position,
        if rows { inset_v } else { inset_h },
        Line {
            start: gm_start,
            end: gm_end,
        },
        0.0,
        0.0,
        Direction::Ltr,
    );
    let stack_inset = if rows { inset_h } else { inset_v };
    let stack_relative = if position == Position::Relative {
        stack_inset
            .start
            .or(stack_inset.end.map(|v| -v))
            .unwrap_or(0.0)
    } else {
        0.0
    };
    let resolved_margin = if rows {
        Rect {
            top: grid_margin.start,
            bottom: grid_margin.end,
            left: margin.left.unwrap_or(0.0),
            right: margin.right.unwrap_or(0.0),
        }
    } else {
        Rect {
            left: grid_margin.start,
            right: grid_margin.end,
            top: margin.top.unwrap_or(0.0),
            bottom: margin.bottom.unwrap_or(0.0),
        }
    };
    let scrollbar_size = Size {
        width: if overflow.y == Overflow::Scroll {
            scrollbar_width
        } else {
            0.0
        },
        height: if overflow.x == Overflow::Scroll {
            scrollbar_width
        } else {
            0.0
        },
    };
    #[cfg(feature = "content_size")]
    let scrollable_overflow_rect = output.scrollable_overflow_rect;
    #[cfg(not(feature = "content_size"))]
    let scrollable_overflow_rect = Rect::ZERO;
    ItemBox {
        size,
        margin: resolved_margin,
        grid_offset,
        stack_relative,
        padding,
        border,
        scrollbar_size,
        scrollable_overflow_rect,
        contain,
        overflow,
        baseline: output.baselines.first,
        baseline_x: output.baselines_x.first,
        last_baseline_x: output.last_or_first_x(),
        baseline_x_flags,
        stack_align: stack_align.filter(|a| {
            !matches!(
                a.keyword,
                AlignItemsKeyword::Baseline | AlignItemsKeyword::LastBaseline
            )
        }),
        stack_size_fixed,
        grid_baseline: grid_alignment.keyword == AlignItemsKeyword::Baseline,
        last_baseline: output.last_or_first_y(),
        grid_last_baseline: grid_alignment.keyword == AlignItemsKeyword::LastBaseline,
    }
}

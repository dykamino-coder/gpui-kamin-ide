//! KaminIDE patch: ПОДСЕТКА (css-grid-2 §9 `#subgrids`) на общем пути сетки.
//!
//! Спека — `refs/csswg-drafts/css-grid-2/Overview.bs:3555-3925`; референс —
//! Blink `third_party/blink/renderer/core/layout/grid/` (`grid_sizing_tree.cc`,
//! `grid_track_collection.cc` `CreateSubgridTrackCollection`,
//! `grid_layout_algorithm.cc` `ComputeSubgridIntrinsicSize`).
//!
//! Устройство (упрощённое дерево размеров Blink):
//! * Родительская сетка после размещения своих элементов «сплющивает» в себя
//!   элементы каждой подсетки-ребёнка (§9 (h) `#subgrid-item-contribution`):
//!   они размещаются ТЕМ ЖЕ кодом, что у самой подсетки (`place_items`), и
//!   вкладывают свои размеры в дорожки родителя по подсеточным осям; края
//!   подсетки и половина разницы зазоров идут им «extra margin»
//!   (§subgrid-margins, §subgrid-gaps), накапливаясь через уровни. Сама
//!   подсетка в подсеточной оси считается пустой (§9 (g)).
//! * Перед каждым замером и перед раскладкой подсетки родитель пишет ей
//!   РАЗРЕШЁННЫЕ размеры дорожек её пролёта (`publish_subgrid_tracks`,
//!   §9 (a) `#subgrid-tracks`). Подсетка с записью берёт их вместо своего
//!   шаблона: явных дорожек ровно по пролёту (§9 (b)), неявных в подсеточной
//!   оси нет — область элемента зажимается (§9 (f) `#subgrid-implicit`),
//!   выравнивание содержимого в этой оси не действует (§subgrid-grid-alignment).
#[path = "fixed_standalone_tracks.rs"]
mod fixed_standalone_tracks;
use fixed_standalone_tracks::standalone_tracks_with_known;
#[path = "nested_standalone_size.rs"]
mod nested_standalone_size;
use super::explicit_grid::{compute_explicit_grid_size_in_axis, AutoRepeatStrategy};
use super::implicit_grid::compute_grid_size_estimate;
use super::placement::{place_grid_items, ItemPlacement};
use super::subgrid_tracks::axis_tracks;
use super::types::{CellOccupancyMatrix, GridItem, GridTrack, NamedLineResolver, TrackCounts};
use super::{OriginZeroLine, MAX_GRID_TRACKS};
use crate::geometry::{AbsoluteAxis, AbstractAxis, InBothAbsAxis, Line, Rect, Size};
use crate::style::{
    AlignItems, AlignItemsKeyword, MaxTrackSizingFunction, MinTrackSizingFunction, Overflow,
    Position, SubgridAxisTracks, SubgridTracks, SUBGRID_COLUMNS, SUBGRID_COLUMN_GAP_NORMAL,
    SUBGRID_ROWS, SUBGRID_ROW_GAP_NORMAL,
};
use crate::tree::{LayoutPartialTreeExt, NodeId};
use crate::util::sys::Vec;
use crate::util::{MaybeResolve, ResolveOrZero};
use crate::GenericRepetition;
use crate::{
    BoxGenerationMode, CoreStyle, GridContainerStyle, GridItemStyle, LayoutGridContainer,
    LengthPercentage,
};

/// Глубина вложенности подсеток, дальше которой сплющивание не идёт
/// (страховка от патологических деревьев; у WPT — не глубже четырёх).
const MAX_DEPTH: u8 = 12;

/// Результат размещения элементов сетки (шаги 2-4 `compute_grid_layout`).
pub(super) struct PlacedGrid<S: crate::CheapCloneStr> {
    /// Элементы в потоке.
    pub items: Vec<GridItem>,
    /// Занятость клеток (для схлопывания `auto-fit`).
    pub cell_occupancy_matrix: CellOccupancyMatrix,
    /// Имена линий.
    pub name_resolver: NamedLineResolver<S>,
    /// Число дорожек колонок.
    pub col_counts: TrackCounts,
    /// Число дорожек рядов.
    pub row_counts: TrackCounts,
    /// Native auto-repeat counts used by track initialization and detailed names.
    pub col_auto_repetition_count: u16,
    pub row_auto_repetition_count: u16,
}

/// Подсеточные оси узла, выданные родителем; оси без бита в стиле
/// отбрасываются (запись могла остаться от прежнего стиля узла).
pub(super) fn own_subgrid_tracks<Tree: LayoutGridContainer>(
    tree: &Tree,
    node: NodeId,
    bits: u8,
) -> Option<SubgridTracks> {
    if bits & (SUBGRID_COLUMNS | SUBGRID_ROWS) == 0 {
        return None;
    }
    let mut tracks = tree.get_subgrid_tracks(node)?;
    if bits & SUBGRID_COLUMNS == 0 {
        tracks.columns = None;
    }
    if bits & SUBGRID_ROWS == 0 {
        tracks.rows = None;
    }
    (tracks.columns.is_some() || tracks.rows.is_some()).then_some(tracks)
}

/// Явная сетка и размещение элементов контейнера `node` — ОДИН код для самой
/// сетки и для родителя, сплющивающего её элементы (иначе вклады и раскладка
/// разъехались бы). `sub` — дорожки, выданные подсетке: в её подсеточной оси
/// явных дорожек ровно `count` (css-grid-2 §9 (b)), и область каждого элемента
/// зажимается в них (§9 (f): «each grid item's grid area is clamped to the
/// subgrid's explicit grid»).
#[allow(clippy::too_many_arguments)]
pub(super) fn place_items<Tree: LayoutGridContainer>(
    tree: &Tree,
    node: NodeId,
    style: &Tree::GridContainerStyle<'_>,
    auto_fit_container_size: Size<Option<f32>>,
    auto_repeat_fit_strategy: Size<AutoRepeatStrategy>,
    sub: Option<&SubgridTracks>,
    align_items: AlignItems,
    justify_items: AlignItems,
) -> PlacedGrid<Tree::CustomIdent> {
    let sub_cols = sub
        .and_then(|s| s.columns.as_ref())
        .map(|a| bounded_subgrid_count(a.count));
    let sub_rows = sub
        .and_then(|s| s.rows.as_ref())
        .map(|a| bounded_subgrid_count(a.count));
    let (col_auto_repetition_count, grid_template_col_count) = match sub_cols {
        Some(count) => (0, count),
        None => compute_explicit_grid_size_in_axis(
            style,
            auto_fit_container_size.width,
            auto_repeat_fit_strategy.width,
            |val, basis| tree.calc(val, basis),
            AbsoluteAxis::Horizontal,
        ),
    };
    let (row_auto_repetition_count, grid_template_row_count) = match sub_rows {
        Some(count) => (0, count),
        None => compute_explicit_grid_size_in_axis(
            style,
            auto_fit_container_size.height,
            auto_repeat_fit_strategy.height,
            |val, basis| tree.calc(val, basis),
            AbsoluteAxis::Vertical,
        ),
    };

    let mut name_resolver =
        NamedLineResolver::new(style, col_auto_repetition_count, row_auto_repetition_count);
    let explicit_col_count = match sub_cols {
        Some(count) => count.min(MAX_GRID_TRACKS),
        None => grid_template_col_count
            .max(name_resolver.area_column_count())
            .min(MAX_GRID_TRACKS),
    };
    let explicit_row_count = match sub_rows {
        Some(count) => count.min(MAX_GRID_TRACKS),
        None => grid_template_row_count
            .max(name_resolver.area_row_count())
            .min(MAX_GRID_TRACKS),
    };
    name_resolver.set_explicit_column_count(explicit_col_count);
    name_resolver.set_explicit_row_count(explicit_row_count);
    // Имена линий подсеточной оси: свой `<line-name-list>` (§subgrid-listing,
    // авто-повтор по пролёту) плюс явные имена родителя на тех же линиях
    // («These names are in addition to any line names specified locally on
    // the subgrid», §9 (d)).
    for (columns, axis) in [
        (true, sub.and_then(|s| s.columns.as_ref())),
        (false, sub.and_then(|s| s.rows.as_ref())),
    ] {
        let Some(axis) = axis else { continue };
        let count = bounded_subgrid_count(axis.count);
        let mut lines: Vec<Vec<Tree::CustomIdent>> = style
            .subgrid_line_names(columns)
            .map(|names| names.expand(count))
            .unwrap_or_default();
        lines.resize_with(count as usize + 1, Vec::new);
        for (line, inherited) in lines.iter_mut().zip(axis.names.iter()) {
            line.extend(
                inherited
                    .iter()
                    .map(|name| Tree::CustomIdent::from(name.as_str())),
            );
        }
        name_resolver.set_subgrid_line_names(columns, &lines);
    }

    // Upstream #1200/#1259: create the items once in document order and resolve the named lines of
    // their placements once; the results drive both the grid size estimate and placement.
    // KaminIDE patch (superseded): the estimate used to miss NAMED edges far outside the explicit
    // grid (`grid-column: x -5`, css-grid-2 §8.3), overflowing the occupancy matrix; upstream now
    // resolves named lines before the estimate, so the separate widening pass is gone.
    let child_count = tree.child_count(node);
    let mut items: Vec<GridItem> = Vec::with_capacity(child_count);
    let mut placements: Vec<ItemPlacement> = Vec::with_capacity(child_count);
    for (index, child_node) in tree.child_ids(node).enumerate() {
        let child_style = tree.get_grid_child_style(child_node);
        if !contributes_to_placement(&child_style) {
            continue;
        }
        placements.push(InBothAbsAxis {
            horizontal: name_resolver
                .resolve_column_names(&child_style.grid_column())
                .into_origin_zero(explicit_col_count),
            vertical: name_resolver
                .resolve_row_names(&child_style.grid_row())
                .into_origin_zero(explicit_row_count),
        });
        items.push(GridItem::new_with_style_and_order(
            child_node,
            child_style,
            align_items,
            justify_items,
            index as u16,
        ));
    }
    let (est_col_counts, est_row_counts) =
        compute_grid_size_estimate(explicit_col_count, explicit_row_count, &placements);

    let mut cell_occupancy_matrix =
        CellOccupancyMatrix::with_track_counts(est_col_counts, est_row_counts);
    place_grid_items(
        &mut cell_occupancy_matrix,
        &mut items,
        &placements,
        style.grid_auto_flow(),
    );

    let mut col_counts = *cell_occupancy_matrix.track_counts(AbsoluteAxis::Horizontal);
    let mut row_counts = *cell_occupancy_matrix.track_counts(AbsoluteAxis::Vertical);
    // §9 (f) `#subgrid-implicit`: «The subgrid does not have any implicit grid
    // tracks in the subgridded dimension(s)… each grid item's grid area is
    // clamped to the subgrid's explicit grid (using the same procedure as for
    // clamping placement in an overly-large grid)» — пример спеки: `2 / span 3`
    // в подсетке `span 1` уходит в её единственную дорожку.
    let clamp = |line: Line<OriginZeroLine>, count: u16| -> Line<OriginZeroLine> {
        let n = count as i16;
        let start = line.start.0.clamp(0, n - 1);
        let end = line.end.0.clamp(start + 1, n);
        Line {
            start: OriginZeroLine(start),
            end: OriginZeroLine(end),
        }
    };
    if let Some(count) = sub_cols {
        for item in items.iter_mut() {
            item.column = clamp(item.column, count);
        }
        col_counts = TrackCounts::from_raw(0, count, 0);
    }
    if let Some(count) = sub_rows {
        for item in items.iter_mut() {
            item.row = clamp(item.row, count);
        }
        row_counts = TrackCounts::from_raw(0, count, 0);
    }

    PlacedGrid {
        items,
        cell_occupancy_matrix,
        name_resolver,
        col_counts,
        row_counts,
        col_auto_repetition_count,
        row_auto_repetition_count,
    }
}

/// Дорожки подсеточной оси из выданных размеров: первая и последняя линии
/// схлопнуты (как у `initialize_grid_tracks`), между дорожками — зазор
/// подсетки. Без размеров (родитель ещё не размерил ось) дорожки `auto`.
pub(super) fn initialize_subgrid_tracks(tracks: &mut Vec<GridTrack>, axis: &SubgridAxisTracks) {
    tracks.clear();
    let count = bounded_subgrid_count(axis.count) as usize;
    tracks.reserve(count * 2 + 1);
    let gutter = || GridTrack::gutter(LengthPercentage::length(axis.gap));
    tracks.push(gutter());
    for i in 0..count {
        let track = match &axis.sizes {
            Some(sizes) => {
                let size = sizes.get(i).copied().unwrap_or(0.0);
                GridTrack::new(
                    MinTrackSizingFunction::length(size),
                    MaxTrackSizingFunction::length(size),
                )
            }
            None => GridTrack::new(
                MinTrackSizingFunction::auto(),
                MaxTrackSizingFunction::auto(),
            ),
        };
        tracks.push(track);
        tracks.push(gutter());
    }
    if let Some(first) = tracks.first_mut() {
        first.collapse();
    }
    if let Some(last) = tracks.last_mut() {
        last.collapse();
    }
}

/// Края подсетки по физическим сторонам: поле + рамка + отбивка (+ жёлоб
/// полосы прокрутки) — css-grid-2 §subgrid-margins: «the sum of the subgrid's
/// margin, padding, scrollbar gutter, and border at each edge». Доли — от
/// ширины области содержимого контейнера (`auto`-поле — ноль).
fn subgrid_edges<Tree: LayoutGridContainer>(
    tree: &Tree,
    style: &Tree::GridContainerStyle<'_>,
    basis: Option<f32>,
) -> Rect<f32> {
    let basis = basis.or(Some(0.0));
    let margin = style
        .margin()
        .map(|m| m.resolve_or_zero(basis, |val, b| tree.calc(val, b)));
    let border = style
        .border()
        .map(|b| b.resolve_or_zero(basis, |val, bb| tree.calc(val, bb)));
    let padding = style
        .padding()
        .map(|p| p.resolve_or_zero(basis, |val, bb| tree.calc(val, bb)));
    let overflow = style.overflow();
    let scrollbar = style.scrollbar_width();
    Rect {
        left: margin.left + border.left + padding.left,
        right: margin.right
            + border.right
            + padding.right
            + if overflow.y == Overflow::Scroll {
                scrollbar
            } else {
                0.0
            },
        top: margin.top + border.top + padding.top,
        bottom: margin.bottom
            + border.bottom
            + padding.bottom
            + if overflow.x == Overflow::Scroll {
                scrollbar
            } else {
                0.0
            },
    }
}

/// Использованный зазор подсетки по осям: `normal` — зазор родителя
/// (css-grid-2 §subgrid-gaps: «A value of `normal` indicates that the subgrid
/// has the same size gutters as its parent grid»).
fn subgrid_gaps<Tree: LayoutGridContainer>(
    tree: &Tree,
    style: &Tree::GridContainerStyle<'_>,
    bits: u8,
    parent_gap: Size<f32>,
    basis: Size<Option<f32>>,
) -> Size<f32> {
    let own = style.gap();
    Size {
        width: if bits & SUBGRID_COLUMN_GAP_NORMAL != 0 {
            parent_gap.width
        } else {
            own.width
                .resolve_or_zero(basis.width, |val, b| tree.calc(val, b))
        },
        height: if bits & SUBGRID_ROW_GAP_NORMAL != 0 {
            parent_gap.height
        } else {
            own.height
                .resolve_or_zero(basis.height, |val, b| tree.calc(val, b))
        },
    }
}

/// Биты подсеточных осей ребёнка, которые сетка вправду связывает со своими
/// дорожками. Лунки-подсетка (`grid-lanes` с `subgrid`) раскладывается своим
/// кодом — её элементы сюда не сплющиваются.
fn linked_axes<Tree: LayoutGridContainer>(tree: &Tree, node: NodeId) -> u8 {
    let style = tree.get_grid_container_style(node);
    if style.grid_lanes().is_some() {
        return 0;
    }
    style.subgrid() & (SUBGRID_COLUMNS | SUBGRID_ROWS)
}

/// Край подсетки в дорожках корневой сетки: накопленный «extra margin» у
/// первой и последней дорожки её пролёта (Blink `StartExtraMargin`/
/// `EndExtraMargin`, `grid_track_collection.cc:558-569`).
pub(super) struct SubgridEdge {
    /// Ось: `true` — колонки.
    columns: bool,
    /// Пролёт подсетки в координатах корневой сетки.
    span: Line<OriginZeroLine>,
    /// Накопленный край у начала пролёта.
    start: f32,
    /// Накопленный край у конца пролёта.
    end: f32,
    /// Разница зазора подсетки и зазора корневой сетки (накопленная,
    /// Blink `AccumulatedGutterSizeDelta`).
    delta: f32,
    /// Подсетка-ребёнок корневого контейнера, под которой лежит эта.
    root: NodeId,
}

/// Сплющить элементы подсеток-детей в дорожки контейнера (css-grid-2 §9 (h)).
///
/// `items` — элементы самого контейнера (после размещения); в конец
/// дописываются сплющенные элементы, а у самой подсетки снимается бит
/// подсеточной оси («acts as if it was completely empty for track sizing
/// purposes in the subgridded dimension», §9 (g)). `container_gap` —
/// использованные зазоры контейнера (точки). `axis_mask` — оси, по которым
/// контейнер вообще связывает подсетки (у лунок — только ось решётки,
/// css-grid-3 Overview.bs:502-519). Возвращает края всех подсеток дерева —
/// для пола дорожек (`apply_subgrid_floors`).
pub(super) fn flatten_subgrid_items<Tree: LayoutGridContainer>(
    tree: &mut Tree,
    items: &mut Vec<GridItem>,
    container_gap: Size<f32>,
    inner_node_size: Size<Option<f32>>,
    axis_mask: u8,
    resolver: Option<&NamedLineResolver<Tree::CustomIdent>>,
    root_axes: Size<bool>,
) -> Vec<SubgridEdge> {
    let own = items.len();
    let mut out: Vec<GridItem> = Vec::new();
    let mut edges: Vec<SubgridEdge> = Vec::new();
    for item in items.iter_mut().take(own) {
        let linked = linked_axes(tree, item.node) & axis_mask;
        if linked == 0 {
            continue;
        }
        item.sizing_axes &= !linked;
        let area = (item.column, item.row);
        let (first_edge, first_item) = (edges.len(), out.len());
        flatten_into(
            tree,
            item.node,
            linked,
            area,
            area,
            root_axes,
            root_axes,
            resolver,
            Rect::ZERO,
            container_gap,
            container_gap,
            inner_node_size,
            Size::NONE,
            &mut out,
            &mut edges,
            0,
        );
        for edge in edges.iter_mut().skip(first_edge) {
            edge.root = item.node;
        }
        for flat in out.iter_mut().skip(first_item) {
            flat.subgrid_root = Some(item.node);
        }
    }
    items.extend(out);
    edges
}

/// Пол дорожек лунок от АВТО-размещённых подсеток (Blink
/// `AccommodateSubgridExtraMargins`, ветка `is_auto_placed`, и
/// `LargestAutoPlacedSubgridContribution`, `grid_layout_utils.cc:1236-1258`;
/// резолюция csswg-drafts#10926): позиция такой подсетки при размере дорожек
/// не известна, и её наибольший вклад краёв в одну дорожку получает КАЖДАЯ
/// дорожка — у подсетки в одну дорожку оба края, в две — край и половина
/// разницы зазоров, шире — ещё и целая разница у внутренних
/// (`column-subgrid-extra-margin-006..009`: четыре колонки по 29).
pub(super) fn apply_lanes_auto_floors(
    tracks: &mut [GridTrack],
    edges: &[SubgridEdge],
    columns: bool,
    auto_root: impl Fn(NodeId) -> bool,
) {
    let mut largest = 0.0f32;
    for edge in edges
        .iter()
        .filter(|e| e.columns == columns && auto_root(e.root))
    {
        let half = edge.delta / 2.0;
        let contribution = match edge.span.span() {
            0 | 1 => edge.start + edge.end,
            2 => (edge.start + half).max(edge.end + half),
            _ => (edge.start + half).max(edge.delta).max(edge.end + half),
        };
        largest = largest.max(contribution);
    }
    if largest <= 0.0 {
        return;
    }
    for (i, track) in tracks.iter_mut().enumerate() {
        if i % 2 == 1 && !track.is_collapsed {
            track.subgrid_floor = track.subgrid_floor.max(largest);
        }
    }
}

/// Пол дорожек оси от краёв подсеток (Blink `AccommodateSubgridExtraMargins`,
/// `grid_layout_utils.cc:1260-1356`): у подсетки с определённой позицией
/// крайняя дорожка пролёта получает свой накопленный край, а при пролёте в
/// одну дорожку — сумму обоих. Так пустая подсетка с полями и отбивками
/// (`subgrid-no-items-on-edges-001`) всё равно раздвигает дорожки родителя.
pub(super) fn apply_subgrid_floors(
    tracks: &mut [GridTrack],
    edges: &[SubgridEdge],
    columns: bool,
    counts: TrackCounts,
) {
    for track in tracks.iter_mut() {
        track.subgrid_floor = 0.0;
    }
    for edge in edges.iter().filter(|e| e.columns == columns) {
        let (Some(start), Some(end)) = (
            edge.span.start.try_into_track_vec_index(counts),
            edge.span.end.try_into_track_vec_index(counts),
        ) else {
            continue;
        };
        if end <= start {
            continue;
        }
        let (first, last) = (start + 1, end - 1);
        let mut floor = |i: usize, v: f32| {
            if let Some(track) = tracks.get_mut(i) {
                track.subgrid_floor = track.subgrid_floor.max(v);
            }
        };
        if first == last {
            floor(first, edge.start + edge.end);
        } else {
            floor(first, edge.start);
            floor(last, edge.end);
        }
    }
}

/// Один уровень сплющивания: элементы подсетки `node`, чья область в
/// координатах КОРНЕВОЙ сетки — `area`, а накопленный «extra margin» самой
/// подсетки — `ext`.
#[allow(clippy::too_many_arguments)]
fn flatten_into<Tree: LayoutGridContainer>(
    tree: &mut Tree,
    node: NodeId,
    linked: u8,
    area: (Line<OriginZeroLine>, Line<OriginZeroLine>),
    local: (Line<OriginZeroLine>, Line<OriginZeroLine>),
    root_axes: Size<bool>,
    parent_axes: Size<bool>,
    parent_resolver: Option<&NamedLineResolver<Tree::CustomIdent>>,
    ext: Rect<f32>,
    parent_gap: Size<f32>,
    root_gap: Size<f32>,
    inner_node_size: Size<Option<f32>>,
    known_content: Size<Option<f32>>,
    out: &mut Vec<GridItem>,
    edges_out: &mut Vec<SubgridEdge>,
    depth: u8,
) {
    if depth >= MAX_DEPTH {
        return;
    }
    let style = tree.get_grid_container_style(node);
    let bits = style.subgrid();
    let own_axes = super::subgrid_flow::axes(&style);
    let edges = subgrid_edges(tree, &style, inner_node_size.width);
    let gap = subgrid_gaps(tree, &style, bits, parent_gap, inner_node_size);
    // Имена — у родителя на линиях пролёта (`local` — пролёт в координатах
    // РОДИТЕЛЯ, его имена считаются от его явной сетки).
    let inherited = |columns: bool, span: Line<OriginZeroLine>| -> Vec<Vec<String>> {
        let mut names = parent_resolver
            .map(|r| r.names_in_span(columns, span.start.0 + 1, span.span().max(1)))
            .unwrap_or_default();
        if if columns {
            own_axes.width != parent_axes.width
        } else {
            own_axes.height != parent_axes.height
        } {
            names.reverse();
        }
        names
    };
    let sub = SubgridTracks {
        columns: (linked & SUBGRID_COLUMNS != 0).then(|| SubgridAxisTracks {
            count: area.0.span().max(1),
            sizes: None,
            gap: gap.width,
            names: inherited(true, local.0),
        }),
        rows: (linked & SUBGRID_ROWS != 0).then(|| SubgridAxisTracks {
            count: area.1.span().max(1),
            sizes: None,
            gap: gap.height,
            names: inherited(false, local.1),
        }),
    };
    // Upstream #1254: `align-items`/`justify-items` are no longer `Option` (default `normal`).
    let align_items = style.align_items();
    let justify_items = style.justify_items();
    // Размер для `repeat(auto-*)` в СВОЕЙ оси подсетки здесь неизвестен —
    // повтор идёт один раз (css-grid-2 §7.2.3.2 «Otherwise, the specified
    // track list repeats only once»). Подсеточная ось повторов не знает.
    let strategy = Size {
        width: AutoRepeatStrategy::MinRepetitionsThatDoOverflow,
        height: AutoRepeatStrategy::MinRepetitionsThatDoOverflow,
    };
    let placed = place_items(
        tree,
        node,
        &style,
        Size::NONE,
        strategy,
        Some(&sub),
        align_items,
        justify_items,
    );
    drop(style);
    let logical_ext = super::subgrid_flow::logical_edges(ext, root_axes);
    let logical_edges = super::subgrid_flow::logical_edges(edges, root_axes);
    if linked & SUBGRID_COLUMNS != 0 {
        edges_out.push(SubgridEdge {
            columns: true,
            span: area.0,
            start: logical_ext.left + logical_edges.left,
            end: logical_ext.right + logical_edges.right,
            delta: gap.width - root_gap.width,
            root: node,
        });
    }
    if linked & SUBGRID_ROWS != 0 {
        edges_out.push(SubgridEdge {
            columns: false,
            span: area.1,
            start: logical_ext.top + logical_edges.top,
            end: logical_ext.bottom + logical_edges.bottom,
            delta: gap.height - root_gap.height,
            root: node,
        });
    }

    // Собственные дорожки НЕподсеточной оси, если они известны без
    // содержимого (`standalone_tracks`): по ним сплющенный элемент меряется
    // поперёк вместо всей области подсетки.
    let style = tree.get_grid_container_style(node);
    let own_cols = (linked & SUBGRID_COLUMNS == 0)
        .then(|| standalone_tracks_with_known(tree, &style, true, inner_node_size, known_content))
        .flatten();
    let own_rows = (linked & SUBGRID_ROWS == 0)
        .then(|| standalone_tracks_with_known(tree, &style, false, inner_node_size, known_content))
        .flatten();
    // KaminIDE patch: см. `GridItem::subgrid_cross_auto` — у НЕподсеточной
    // оси без шаблона и без `grid-auto-*` дорожки неявные `auto`.
    let implicit_auto = |columns: bool| {
        let (template, auto) = if columns {
            (
                style.grid_template_columns().map_or(true, |t| t.len() == 0),
                style.grid_auto_columns().len() == 0,
            )
        } else {
            (
                style.grid_template_rows().map_or(true, |t| t.len() == 0),
                style.grid_auto_rows().len() == 0,
            )
        };
        template && auto
    };
    let cross_auto = Size {
        width: linked & SUBGRID_COLUMNS == 0 && own_cols.is_none() && implicit_auto(true),
        height: false,
    };
    drop(style);
    let PlacedGrid {
        items: placed_items,
        name_resolver: own_resolver,
        ..
    } = placed;
    let span_cols = area.0.span().max(1) as i16;
    let span_rows = area.1.span().max(1) as i16;
    for mut child in placed_items {
        let chained = linked_axes(tree, child.node) & linked;
        let column = if linked & SUBGRID_COLUMNS != 0 {
            super::subgrid_flow::project(child.column, area.0, own_axes.width != root_axes.width)
        } else {
            area.0
        };
        let row = if linked & SUBGRID_ROWS != 0 {
            super::subgrid_flow::project(child.row, area.1, own_axes.height != root_axes.height)
        } else {
            area.1
        };
        // §subgrid-margins: у крайних элементов — края подсетки поверх
        // накопленного; §subgrid-gaps: у внутренних сторон — половина разницы
        // зазоров. Разница телескопична через уровни: внутренняя линия
        // вложенной подсетки — внутренняя и у внешней, и итог равен половине
        // разницы СВОЕГО зазора и зазора корневой сетки.
        //
        // В НЕподсеточной оси элемент лежит в собственных дорожках подсетки, а
        // сплющенному известна лишь вся её область: края подсетки с обеих
        // сторон сужают эту область до коробки содержимого — по ней элемент
        // и меряется поперёк (перенос строк при растяжке). В размер дорожек
        // этой оси он не вкладывается (`sizing_axes`).
        let mut extra = Rect {
            left: ext.left + edges.left,
            right: ext.right + edges.right,
            top: ext.top + edges.top,
            bottom: ext.bottom + edges.bottom,
        };
        if linked & SUBGRID_COLUMNS != 0 {
            let inner = (gap.width - root_gap.width) / 2.0;
            super::subgrid_flow::inner_edges(
                &mut extra,
                child.column,
                span_cols,
                inner,
                true,
                own_axes.width,
            );
        }
        if linked & SUBGRID_ROWS != 0 {
            let inner = (gap.height - root_gap.height) / 2.0;
            super::subgrid_flow::inner_edges(
                &mut extra,
                child.row,
                span_rows,
                inner,
                false,
                own_axes.height,
            );
        }
        child.subgrid_cross_auto = cross_auto;
        if let Some(width) = nested_standalone_size::cross(&own_cols, child.column) {
            child.subgrid_cross.width = Some(width);
            extra.left = 0.0;
            extra.right = 0.0;
        }
        if let Some(height) = nested_standalone_size::cross(&own_rows, child.row) {
            child.subgrid_cross.height = Some(height);
            extra.top = 0.0;
            extra.bottom = 0.0;
        }
        if chained != 0 {
            let child_content = nested_standalone_size::content(tree, &child, chained);
            flatten_into(
                tree,
                child.node,
                chained,
                (column, row),
                (child.column, child.row),
                root_axes,
                own_axes,
                Some(&own_resolver),
                extra,
                gap,
                root_gap,
                inner_node_size,
                child_content,
                out,
                edges_out,
                depth + 1,
            );
        }
        let sizing = linked & !chained;
        if sizing == 0 {
            continue;
        }
        child.column = column;
        child.row = row;
        child.sizing_axes = sizing;
        child.flattened = true;
        child.extra_margin = extra;
        child.source_order = u16::MAX;
        // Общие базовые линии через подсетку — отдельный механизм (Blink
        // `GridBaselineAccumulator` по дереву размеров); здесь элемент
        // вкладывает только размеры.
        if child.align_self.keyword == AlignItemsKeyword::Baseline {
            child.align_self.keyword = AlignItemsKeyword::Start;
        }
        if child.justify_self.keyword == AlignItemsKeyword::Baseline {
            child.justify_self.keyword = AlignItemsKeyword::Start;
        }
        out.push(child);
    }
}

/// Размеры записей вектора дорожек, известные ДО алгоритма дорожек: у
/// записи с одинаковыми определёнными гранями (`100px`, зазор) и у
/// схлопнутой. Подсетку меряют и до размера дорожек родителя (вклад во
/// вторую ось, мерка лунок по оси укладки) — фиксированные дорожки ей
/// отдаются сразу (у прежнего среза `dom.rs` они были всегда).
pub(super) fn fixed_track_sizes<Tree: LayoutGridContainer>(
    tree: &Tree,
    tracks: &[GridTrack],
    basis: Option<f32>,
) -> Vec<Option<f32>> {
    tracks
        .iter()
        .map(|t| {
            if t.is_collapsed {
                return Some(0.0);
            }
            let min = t
                .min_track_sizing_function
                .definite_value(basis, |val, b| tree.calc(val, b));
            let max = t
                .max_track_sizing_function
                .definite_value(basis, |val, b| tree.calc(val, b));
            match (min, max) {
                (Some(a), Some(b)) if (a - b).abs() < 0.001 => Some(a),
                _ => None,
            }
        })
        .collect()
}

/// Записать подсеткам-детям разрешённые дорожки их пролёта (css-grid-2 §9 (a)).
///
/// Зовётся перед каждым проходом, где подсетку меряют, и перед раскладкой:
/// `sized` — какие оси контейнера уже размерены (неразмеренная ось уходит
/// без размеров), `use_offsets` — дорожки уже выровнены (`align_tracks`), и
/// позиции берутся из смещений, а не из суммы размеров.
#[allow(clippy::too_many_arguments)]
pub(super) fn publish_subgrid_tracks<Tree: LayoutGridContainer>(
    tree: &mut Tree,
    items: &[GridItem],
    columns: &[GridTrack],
    rows: &[GridTrack],
    sized: (bool, bool),
    use_offsets: bool,
    container_gap: Size<f32>,
    inner_node_size: Size<Option<f32>>,
    resolver: &NamedLineResolver<Tree::CustomIdent>,
    parent_axes: Size<bool>,
) {
    for item in items.iter().filter(|item| !item.flattened) {
        let linked = linked_axes(tree, item.node);
        if linked == 0 {
            continue;
        }
        let style = tree.get_grid_container_style(item.node);
        let child_axes = super::subgrid_flow::axes(&style);
        let bits = style.subgrid();
        let edges = subgrid_edges(tree, &style, inner_node_size.width);
        let gap = subgrid_gaps(tree, &style, bits, container_gap, inner_node_size);
        drop(style);
        let fixed_cols = (!sized.0 && linked & SUBGRID_COLUMNS != 0)
            .then(|| fixed_track_sizes(tree, columns, inner_node_size.width));
        let fixed_rows = (!sized.1 && linked & SUBGRID_ROWS != 0)
            .then(|| fixed_track_sizes(tree, rows, inner_node_size.height));
        let tracks = SubgridTracks {
            columns: (linked & SUBGRID_COLUMNS != 0).then(|| {
                axis_tracks(
                    columns,
                    item.column_indexes,
                    edges.left,
                    edges.right,
                    gap.width,
                    sized.0,
                    use_offsets,
                    fixed_cols.as_deref(),
                    resolver.names_in_span(
                        true,
                        item.column.start.0 + 1,
                        item.column.span().max(1),
                    ),
                    parent_axes.width,
                    child_axes.width,
                )
            }),
            rows: (linked & SUBGRID_ROWS != 0).then(|| {
                axis_tracks(
                    rows,
                    item.row_indexes,
                    edges.top,
                    edges.bottom,
                    gap.height,
                    sized.1,
                    use_offsets,
                    fixed_rows.as_deref(),
                    resolver.names_in_span(false, item.row.start.0 + 1, item.row.span().max(1)),
                    parent_axes.height,
                    child_axes.height,
                )
            }),
        };
        tree.set_subgrid_tracks(item.node, Some(tracks));
    }
}

/// Переставить элементы так, чтобы вкладывающиеся в ось `axis` стояли
/// первыми, и вернуть их число: алгоритм размеров дорожек получает срез
/// `items[..k]`. Перестановка устойчивая; исходный порядок восстанавливает
/// сортировка по `source_order` перед раскладкой.
pub(super) fn partition_for_axis(items: &mut [GridItem], axis: AbstractAxis) -> usize {
    if items.iter().all(|item| item.sizes_axis(axis)) {
        return items.len();
    }
    items.sort_by_key(|item| !item.sizes_axis(axis));
    items
        .iter()
        .take_while(|item| item.sizes_axis(axis))
        .count()
}

/// Записать подсетке-элементу ЛУНОК дорожки оси решётки её пролёта
/// (css-grid-3 Overview.bs:502-519: в лунках подсеточной бывает только ось
/// решётки). `lines` — индексы линий пролёта в векторе дорожек оси решётки;
/// `sized` — дорожки уже размерены и выровнены.
#[allow(clippy::too_many_arguments)]
pub(super) fn publish_lanes_subgrid<Tree: LayoutGridContainer>(
    tree: &mut Tree,
    node: NodeId,
    rows: bool,
    tracks: &[GridTrack],
    lines: Line<u16>,
    sized: bool,
    container_gap: f32,
    inner_node_size: Size<Option<f32>>,
    names: Vec<Vec<String>>,
    parent_axes: Size<bool>,
) {
    let bit = if rows { SUBGRID_ROWS } else { SUBGRID_COLUMNS };
    if linked_axes(tree, node) & bit == 0 {
        return;
    }
    let style = tree.get_grid_container_style(node);
    let child_axes = super::subgrid_flow::axes(&style);
    let bits = style.subgrid();
    let edges = subgrid_edges(tree, &style, inner_node_size.width);
    let gap = subgrid_gaps(
        tree,
        &style,
        bits,
        Size {
            width: container_gap,
            height: container_gap,
        },
        inner_node_size,
    );
    drop(style);
    let basis = if rows {
        inner_node_size.height
    } else {
        inner_node_size.width
    };
    let fixed = (!sized).then(|| fixed_track_sizes(tree, tracks, basis));
    let tracks = if rows {
        SubgridTracks {
            columns: None,
            rows: Some(axis_tracks(
                tracks,
                lines,
                edges.top,
                edges.bottom,
                gap.height,
                sized,
                true,
                fixed.as_deref(),
                names,
                parent_axes.height,
                child_axes.height,
            )),
        }
    } else {
        SubgridTracks {
            columns: Some(axis_tracks(
                tracks,
                lines,
                edges.left,
                edges.right,
                gap.width,
                sized,
                true,
                fixed.as_deref(),
                names,
                parent_axes.width,
                child_axes.width,
            )),
            rows: None,
        }
    };
    tree.set_subgrid_tracks(node, Some(tracks));
}

/// Связана ли подсеточная ось узла с осью решётки лунок (для растяжки
/// элемента по §subgrid-box-alignment).
pub(super) fn lanes_subgridded<Tree: LayoutGridContainer>(
    tree: &Tree,
    node: NodeId,
    rows: bool,
) -> bool {
    let bit = if rows { SUBGRID_ROWS } else { SUBGRID_COLUMNS };
    linked_axes(tree, node) & bit != 0
}

/// Keep placement, name expansion and track initialization in the same signed-coordinate bounds.
fn bounded_subgrid_count(count: u16) -> u16 {
    count.clamp(1, MAX_GRID_TRACKS)
}

/// Hidden/abspos children never create implicit tracks, including named-line estimate widening.
fn contributes_to_placement(style: &impl CoreStyle) -> bool {
    style.box_generation_mode() != BoxGenerationMode::None && style.position() != Position::Absolute
}

#[cfg(test)]
mod kamin_placement_tests {
    use super::*;
    use crate::style::Display;
    type Style = crate::style::Style;

    #[test]
    fn absolute_and_hidden_styles_do_not_contribute_to_placement() {
        let mut style = Style::DEFAULT;
        style.position = Position::Absolute;
        assert!(!contributes_to_placement(&style));
        style.position = Position::Relative;
        style.display = Display::None;
        assert!(!contributes_to_placement(&style));
    }

    #[test]
    fn visible_inflow_style_contributes_to_placement() {
        assert!(contributes_to_placement(&Style::DEFAULT));
    }

    #[test]
    fn oversized_inherited_span_keeps_positive_signed_coordinates() {
        let count = bounded_subgrid_count(u16::MAX);
        assert_eq!(count, MAX_GRID_TRACKS);
        assert!(count as i16 > 0);
        assert_eq!(bounded_subgrid_count(0), 1);
        assert_eq!(bounded_subgrid_count(12), 12);
    }
}

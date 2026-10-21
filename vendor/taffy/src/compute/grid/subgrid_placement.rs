//! Subgrid placement phase. CSS Grid 2 §9.

use super::super::explicit_grid::compute_explicit_grid_size_in_axis;
use super::super::explicit_grid::AutoRepeatStrategy;
use super::super::implicit_grid::compute_grid_size_estimate;
use super::super::placement::place_grid_items;
use super::super::placement::ItemPlacement;
use super::super::types::CellOccupancyMatrix;
use super::super::types::GridItem;
use super::super::types::NamedLineResolver;
use super::super::types::TrackCounts;
use super::super::OriginZeroLine;
use super::super::MAX_GRID_TRACKS;
use super::PlacedGrid;
use crate::geometry::AbsoluteAxis;
use crate::geometry::InBothAbsAxis;
use crate::geometry::Line;
use crate::geometry::Size;
use crate::style::AlignItems;
use crate::style::Position;
use crate::style::SubgridTracks;
use crate::tree::LayoutPartialTreeExt;
use crate::tree::NodeId;
use crate::util::sys::Vec;
use crate::BoxGenerationMode;
use crate::CoreStyle;
use crate::GridContainerStyle;
use crate::GridItemStyle;
use crate::LayoutGridContainer;

/// Явная сетка и размещение элементов контейнера `node` — ОДИН код для самой
/// сетки и для родителя, сплющивающего её элементы (иначе вклады и раскладка
/// разъехались бы). `sub` — дорожки, выданные подсетке: в её подсеточной оси
/// явных дорожек ровно `count` (css-grid-2 §9 (b)), и область каждого элемента
/// зажимается в них (§9 (f): «each grid item's grid area is clamped to the
/// subgrid's explicit grid»).
#[allow(clippy::too_many_arguments)]
pub(in super::super) fn place_items<Tree: LayoutGridContainer>(
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
    // KaminIDE patch: (superseded) the estimate used to miss NAMED edges far outside the explicit
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

/// Hidden/abspos children never create implicit tracks, including named-line estimate widening.
pub(in super::super) fn contributes_to_placement(style: &impl CoreStyle) -> bool {
    style.box_generation_mode() != BoxGenerationMode::None && style.position() != Position::Absolute
}

/// Keep placement, name expansion and track initialization in the same signed-coordinate bounds.
pub(in super::super) fn bounded_subgrid_count(count: u16) -> u16 {
    count.clamp(1, MAX_GRID_TRACKS)
}

//! Subgrid publish phase. CSS Grid 2 §9.

use super::super::subgrid_tracks::axis_tracks;
use super::super::types::GridItem;
use super::super::types::GridTrack;
use super::super::types::NamedLineResolver;
use super::{fixed_track_sizes, linked_axes, subgrid_edges, subgrid_gaps};
use crate::geometry::Line;
use crate::geometry::Size;
use crate::style::SubgridTracks;
use crate::style::SUBGRID_COLUMNS;
use crate::style::SUBGRID_ROWS;
use crate::tree::NodeId;
use crate::util::sys::Vec;
use crate::CoreStyle;
use crate::LayoutGridContainer;

/// Записать подсеткам-детям разрешённые дорожки их пролёта (css-grid-2 §9 (a)).
///
/// Зовётся перед каждым проходом, где подсетку меряют, и перед раскладкой:
/// `sized` — какие оси контейнера уже размерены (неразмеренная ось уходит
/// без размеров), `use_offsets` — дорожки уже выровнены (`align_tracks`), и
/// позиции берутся из смещений, а не из суммы размеров.
#[allow(clippy::too_many_arguments)]
pub(in super::super) fn publish_subgrid_tracks<Tree: LayoutGridContainer>(
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
        let child_axes = super::super::subgrid_flow::axes(&style);
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

/// Записать подсетке-элементу ЛУНОК дорожки оси решётки её пролёта
/// (css-grid-3 Overview.bs:502-519: в лунках подсеточной бывает только ось
/// решётки). `lines` — индексы линий пролёта в векторе дорожек оси решётки;
/// `sized` — дорожки уже размерены и выровнены.
#[allow(clippy::too_many_arguments)]
pub(in super::super) fn publish_lanes_subgrid<Tree: LayoutGridContainer>(
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
    let child_axes = super::super::subgrid_flow::axes(&style);
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
pub(in super::super) fn lanes_subgridded<Tree: LayoutGridContainer>(
    tree: &Tree,
    node: NodeId,
    rows: bool,
) -> bool {
    let bit = if rows { SUBGRID_ROWS } else { SUBGRID_COLUMNS };
    linked_axes(tree, node) & bit != 0
}

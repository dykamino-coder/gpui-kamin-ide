//! Subgrid setup phase. CSS Grid 2 §9.

use super::super::types::NamedLineResolver;
use super::super::OriginZeroLine;
use super::SubgridEdge;
use crate::geometry::Line;
use crate::geometry::Rect;
use crate::geometry::Size;
use crate::style::SubgridAxisTracks;
use crate::style::SubgridTracks;
use crate::style::SUBGRID_COLUMNS;
use crate::style::SUBGRID_ROWS;
use crate::tree::NodeId;
use crate::util::sys::Vec;

/// KaminIDE patch: Prepare inherited subgrid tracks and line names.
pub(in super::super) fn tracks<S: crate::CheapCloneStr>(
    linked: u8,
    area: (Line<OriginZeroLine>, Line<OriginZeroLine>),
    local: (Line<OriginZeroLine>, Line<OriginZeroLine>),
    gap: Size<f32>,
    parent_resolver: Option<&NamedLineResolver<S>>,
    own_axes: Size<bool>,
    parent_axes: Size<bool>,
) -> SubgridTracks {
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
    SubgridTracks {
        columns: (linked & SUBGRID_COLUMNS != 0).then(|| SubgridAxisTracks {
            count: area.0.span().max(1),
            sizes: None,
            gap: gap.width,
            names: inherited(true, local.0),
            growth: Vec::new(),
        }),
        rows: (linked & SUBGRID_ROWS != 0).then(|| SubgridAxisTracks {
            count: area.1.span().max(1),
            sizes: None,
            gap: gap.height,
            names: inherited(false, local.1),
            growth: Vec::new(),
        }),
    }
}

#[allow(clippy::too_many_arguments)]
/// KaminIDE patch: Record the subgrid's edge contributions for inherited tracks.
pub(in super::super) fn record_edges(
    linked: u8,
    area: (Line<OriginZeroLine>, Line<OriginZeroLine>),
    root_axes: Size<bool>,
    ext: Rect<f32>,
    edges: Rect<f32>,
    gap: Size<f32>,
    root_gap: Size<f32>,
    node: NodeId,
    edges_out: &mut Vec<SubgridEdge>,
) {
    let logical_ext = super::super::subgrid_flow::logical_edges(ext, root_axes);
    let logical_edges = super::super::subgrid_flow::logical_edges(edges, root_axes);
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
}

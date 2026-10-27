//! Map inherited grid lines and track order across different physical flow directions.
//! CSS Grid 2 subgrids inherit the parent's tracks, with their own writing-mode ordering.
use super::types::GridTrack;
use super::types::OriginZeroLine;
use crate::geometry::{Line, Rect, Size};
use crate::{CoreStyle, SubgridAxisTracks};

/// KaminIDE patch: Determine the subgrid's physical axis reversals.
pub(super) fn axes(style: &impl CoreStyle) -> Size<bool> {
    style.grid_axis_reversed().unwrap_or(Size {
        width: style.direction().is_rtl(),
        height: false,
    })
}

/// KaminIDE patch: Project a subgrid area into the requested coordinate system.
pub(super) fn area(
    columns: &[GridTrack],
    rows: &[GridTrack],
    column: Line<u16>,
    row: Line<u16>,
    reversed: Size<bool>,
) -> Rect<f32> {
    let x = super::lanes_geometry::track_area(
        columns,
        column.start as usize / 2,
        column.end as usize / 2,
        reversed.width,
    );
    let y = super::lanes_geometry::track_area(
        rows,
        row.start as usize / 2,
        row.end as usize / 2,
        reversed.height,
    );
    Rect {
        left: x.start,
        right: x.end,
        top: y.start,
        bottom: y.end,
    }
}

/// KaminIDE patch: Project inherited tracks into the child's axis order.
pub(super) fn project(
    line: Line<OriginZeroLine>,
    area: Line<OriginZeroLine>,
    reversed: bool,
) -> Line<OriginZeroLine> {
    let span = area.end.0 - area.start.0;
    let (start, end) = if reversed {
        (span - line.end.0, span - line.start.0)
    } else {
        (line.start.0, line.end.0)
    };
    Line {
        start: OriginZeroLine(area.start.0 + start),
        end: OriginZeroLine(area.start.0 + end),
    }
}

/// KaminIDE patch: Reverse inherited track data when the subgrid axis is reversed.
pub(super) fn inherit(mut tracks: SubgridAxisTracks, reversed: bool) -> SubgridAxisTracks {
    if reversed {
        if let Some(sizes) = tracks.sizes.as_mut() {
            sizes.reverse();
        }
        tracks.names.reverse();
        tracks.growth.reverse();
    }
    tracks
}

/// KaminIDE patch: Project physical edges along the subgrid's reversed axes.
pub(super) fn logical_edges(edges: Rect<f32>, reversed: Size<bool>) -> Rect<f32> {
    Rect {
        left: if reversed.width {
            edges.right
        } else {
            edges.left
        },
        right: if reversed.width {
            edges.left
        } else {
            edges.right
        },
        top: if reversed.height {
            edges.bottom
        } else {
            edges.top
        },
        bottom: if reversed.height {
            edges.top
        } else {
            edges.bottom
        },
    }
}

/// KaminIDE patch: Resolve the subgrid's inner edge adjustments.
pub(super) fn inner_edges(
    extra: &mut Rect<f32>,
    line: Line<OriginZeroLine>,
    span: i16,
    inner: f32,
    columns: bool,
    reversed: bool,
) {
    let (lead, trail) = if columns {
        if reversed {
            (&mut extra.right, &mut extra.left)
        } else {
            (&mut extra.left, &mut extra.right)
        }
    } else if reversed {
        (&mut extra.bottom, &mut extra.top)
    } else {
        (&mut extra.top, &mut extra.bottom)
    };
    if line.start.0 > 0 {
        *lead = inner;
    }
    if line.end.0 < span {
        *trail = inner;
    }
}

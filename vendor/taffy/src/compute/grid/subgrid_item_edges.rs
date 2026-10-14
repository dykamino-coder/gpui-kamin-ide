//! Logical edge and gap contributions inherited through a subgrid item.
use super::super::types::GridItem;
use super::super::OriginZeroLine;
use crate::geometry::{Line, Rect, Size};
use crate::{SUBGRID_COLUMNS, SUBGRID_ROWS};

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve(
    ext: Rect<f32>,
    edges: Rect<f32>,
    gap: Size<f32>,
    root_gap: Size<f32>,
    linked: u8,
    child: &GridItem,
    area: (Line<OriginZeroLine>, Line<OriginZeroLine>),
    own_axes: Size<bool>,
) -> Rect<f32> {
    let span_cols = area.0.span().max(1) as i16;
    let span_rows = area.1.span().max(1) as i16;
    let mut extra = Rect {
        left: ext.left + edges.left,
        right: ext.right + edges.right,
        top: ext.top + edges.top,
        bottom: ext.bottom + edges.bottom,
    };
    if linked & SUBGRID_COLUMNS != 0 {
        let inner = (gap.width - root_gap.width) / 2.0;
        super::super::subgrid_flow::inner_edges(
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
        super::super::subgrid_flow::inner_edges(
            &mut extra,
            child.row,
            span_rows,
            inner,
            false,
            own_axes.height,
        );
    }
    extra
}

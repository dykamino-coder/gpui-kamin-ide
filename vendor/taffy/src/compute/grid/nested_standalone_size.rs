//! Definite nested standalone content sizes use the ordinary grid-item sizing rules.
use super::super::OriginZeroLine;
use super::super::types::GridItem;
use crate::geometry::{Line, Size};
use crate::tree::LayoutPartialTreeExt;
use crate::util::ResolveOrZero;
use crate::util::sys::Vec;
use crate::{LayoutGridContainer, SUBGRID_COLUMNS, SUBGRID_ROWS};

pub(super) fn content(
    tree: &mut impl LayoutGridContainer,
    item: &GridItem,
    linked: u8,
) -> Size<Option<f32>> {
    let area = item.subgrid_cross;
    let known = item.known_dimensions(tree, area);
    let padding = item
        .padding
        .resolve_or_zero(area.width, |v, b| tree.calc(v, b));
    let border = item
        .border
        .resolve_or_zero(area.width, |v, b| tree.calc(v, b));
    let edges = (padding + border).sum_axes();
    Size {
        width: if linked & SUBGRID_COLUMNS == 0 {
            known.width.map(|v| (v - edges.width).max(0.0))
        } else {
            None
        },
        height: if linked & SUBGRID_ROWS == 0 {
            known.height.map(|v| (v - edges.height).max(0.0))
        } else {
            None
        },
    }
}

pub(super) fn cross(own: &Option<(Vec<f32>, f32)>, line: Line<OriginZeroLine>) -> Option<f32> {
    let (sizes, gap) = own.as_ref()?;
    let (s, e) = (line.start.0, line.end.0);
    if s < 0 || e as usize > sizes.len() || e <= s {
        return None;
    }
    Some(sizes[s as usize..e as usize].iter().sum::<f32>() + gap * (e - s - 1) as f32)
}

#[cfg(test)]
#[path = "nested_standalone_size_tests.rs"]
mod tests;

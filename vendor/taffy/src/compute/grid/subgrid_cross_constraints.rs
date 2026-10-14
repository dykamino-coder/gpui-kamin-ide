//! Resolve a definite standalone track before flattened item contribution probes.
use super::super::types::GridItem;
use super::standalone_tracks_with_known;
use crate::geometry::Size;
use crate::tree::{LayoutPartialTree, LayoutPartialTreeExt};
use crate::util::{sys::Vec, ResolveOrZero};
use crate::GridContainerStyle;

pub(super) fn own_tracks(
    tree: &impl LayoutPartialTree,
    style: &impl GridContainerStyle,
    columns: bool,
    parent: Size<Option<f32>>,
    known: Size<Option<f32>>,
    items: &[GridItem],
) -> Option<(Vec<f32>, f32)> {
    standalone_tracks_with_known(tree, style, columns, parent, known).or_else(|| {
        let (size, template_empty, auto_empty, gap) = if columns {
            (
                known.width,
                style.grid_template_columns().map_or(true, |t| t.len() == 0),
                style.grid_auto_columns().len() == 0,
                style.gap().width,
            )
        } else {
            (
                known.height,
                style.grid_template_rows().map_or(true, |t| t.len() == 0),
                style.grid_auto_rows().len() == 0,
                style.gap().height,
            )
        };
        let size = size?;
        if !template_empty
            || !auto_empty
            || items.is_empty()
            || !items.iter().all(|item| {
                let line = if columns { item.column } else { item.row };
                line.start.0 == 0 && line.end.0 == 1
            })
        {
            return None;
        }
        Some((
            vec![size.max(0.0)],
            gap.resolve_or_zero(Some(size), |v, b| tree.calc(v, b)),
        ))
    })
}

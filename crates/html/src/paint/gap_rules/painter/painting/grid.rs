//! Grid rule runs use the same logical slots for ink and crossing widths.

use crate::paint::gap_rules::geometry::{GapItem, GapRun, GridTracks, grid_runs};
use crate::paint::gap_rules::{GapAxisRule, GapRuleSpec};
use gpui::{Bounds, Pixels};

pub(super) fn layers<'a>(
    items: &[Bounds<Pixels>],
    spec: &'a GapRuleSpec,
    tracks: Option<&GridTracks>,
) -> Vec<(bool, GapRun, &'a GapAxisRule, bool)> {
    let (on_x, on_y) = if spec.vertical {
        (spec.row.as_ref(), spec.col.as_ref())
    } else {
        (spec.col.as_ref(), spec.row.as_ref())
    };
    let ix: Vec<_> = items
        .iter()
        .map(|b| GapItem::from_bounds(b, true))
        .collect();
    let iy: Vec<_> = items
        .iter()
        .map(|b| GapItem::from_bounds(b, false))
        .collect();
    let (tx, ty) = (spec.tracks_x.as_deref(), spec.tracks_y.as_deref());
    let abs_x = tracks.map(|(c, r)| (c.as_slice(), r.as_slice()));
    let abs_y = tracks.map(|(c, r)| (r.as_slice(), c.as_slice()));
    let mut layers = vec![];
    for (on_x_axis, items, gap_a, gap_b, rule, cross, ta, tb, abs, reverse) in [
        (
            true,
            &ix,
            spec.gap_x,
            spec.gap_y,
            on_x,
            on_y,
            tx,
            ty,
            abs_x,
            (spec.rev_x, spec.rev_y),
        ),
        (
            false,
            &iy,
            spec.gap_y,
            spec.gap_x,
            on_y,
            on_x,
            ty,
            tx,
            abs_y,
            (spec.rev_y, spec.rev_x),
        ),
    ] {
        if let Some(rule) = rule {
            layers.extend(
                grid_runs(items, gap_a, gap_b, rule, cross, ta, tb, abs, reverse)
                    .into_iter()
                    .map(|run| (on_x_axis, run, rule, false)),
            );
        }
    }
    layers
}

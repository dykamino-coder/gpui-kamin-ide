//! Export grid baselines from final child layouts, never track-sizing shims.
//! CSS Grid 2 grid-baselines: sharing preference, available child set, synthesis.

use super::types::GridItem;
use crate::{AlignItemsKeyword, Baselines};

pub(super) fn compute(items: &[GridItem]) -> Baselines {
    Baselines {
        first: baseline(items, false),
        last: baseline(items, true),
    }
}

fn select(
    items: &[GridItem],
    accepts: impl Fn(&GridItem) -> bool,
    last: bool,
) -> Option<&GridItem> {
    let order = |item: &&GridItem| (item.row.start.0, item.column.start.0, item.source_order);
    if last {
        items.iter().filter(|item| accepts(item)).max_by_key(order)
    } else {
        items.iter().filter(|item| accepts(item)).min_by_key(order)
    }
}

fn measured(item: &GridItem, last: bool) -> Option<f32> {
    if last {
        item.last_baseline.or(item.first_baseline)
    } else {
        item.first_baseline.or(item.last_baseline)
    }
}

fn coordinate(item: &GridItem, last: bool) -> f32 {
    let baseline = measured(item, last).unwrap_or(item.height);
    let baseline = if item.overflow.y.is_scroll_container() {
        baseline.clamp(0.0, item.height.max(0.0))
    } else {
        baseline
    };
    item.y_position + baseline
}

fn baseline(items: &[GridItem], last: bool) -> Option<f32> {
    let edge = if last {
        items.iter().map(|item| item.row.end.0).max()?
    } else {
        items.iter().map(|item| item.row.start.0).min()?
    };
    let at_edge = |item: &GridItem| {
        if last {
            item.row.end.0 == edge
        } else {
            item.row.start.0 == edge
        }
    };
    let wanted = if last {
        AlignItemsKeyword::LastBaseline
    } else {
        AlignItemsKeyword::Baseline
    };
    if let Some(item) = select(
        items,
        |item| {
            at_edge(item)
                && item.align_self.keyword == wanted
                && item.participates_in_baseline_alignment()
        },
        last,
    ) {
        return Some(coordinate(item, last));
    }
    // The opposite sharing preference contributes only if its span ends/starts
    // in the indicated track, rather than merely intersecting that track.
    let opposite = if last {
        AlignItemsKeyword::Baseline
    } else {
        AlignItemsKeyword::LastBaseline
    };
    let opposite_edge = |item: &GridItem| {
        if last {
            item.row.start.0 == edge - 1
        } else {
            item.row.end.0 == edge + 1
        }
    };
    if let Some(item) = select(
        items,
        |item| {
            opposite_edge(item)
                && item.align_self.keyword == opposite
                && item.participates_in_baseline_alignment()
        },
        last,
    ) {
        return Some(coordinate(item, !last));
    }
    if let Some(item) = select(
        items,
        |item| at_edge(item) && measured(item, last).is_some(),
        last,
    ) {
        return Some(coordinate(item, last));
    }
    // No child in that track has a usable set: synthesize from the first/last
    // item in grid order, independently of vector sorting during track sizing.
    select(items, |_| true, last).map(|item| item.y_position + item.height)
}

#[cfg(test)]
#[path = "container_baselines_tests.rs"]
mod tests;

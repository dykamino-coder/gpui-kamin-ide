//! Export content baselines from the same child trees and fragment positions used for paint.

use crate::layout::fragment::types::{Frag, Kid, StackChild};
use crate::layout::multicol::column_stack::ColumnStack;
use gpui::{App, AvailableSpace, LayoutMeasurement, Pixels, Window, px, size};
use std::collections::BTreeMap;

#[path = "column_repeat_baselines.rs"]
mod repeated;

pub(super) struct Measurements {
    children: Vec<Vec<LayoutMeasurement>>,
    relative_y: Vec<f32>,
    repeated: Vec<repeated::RepeatedMeasurements>,
}

impl Measurements {
    pub(super) fn new(children: &mut [StackChild], window: &mut Window, cx: &mut App) -> Self {
        let relative_y = children.iter().map(|child| child.rel.1).collect();
        let repeated = children
            .iter_mut()
            .map(|child| repeated::RepeatedMeasurements::new(child, window, cx))
            .collect();
        let children = children
            .iter_mut()
            .map(|child| {
                std::iter::once(&mut child.el)
                    .chain(child.frags.iter_mut())
                    .map(|element| element.snapshot_layout_measurement(window, cx))
                    .collect()
            })
            .collect();
        Self {
            children,
            relative_y,
            repeated,
        }
    }

    pub(super) fn measure(
        &mut self,
        probe: &ColumnStack,
        kids: &[Kid],
        width: f32,
        lines: &[(f32, f32)],
        plan: &[Frag],
        spans: &[(usize, f32)],
        window: &mut Window,
        cx: &mut App,
    ) -> (Option<Pixels>, Option<Pixels>) {
        let col_width =
            ((width - probe.gap * (probe.count as f32 - 1.0)) / probe.count as f32).max(1.0);
        let mut columns: BTreeMap<usize, Baselines> = BTreeMap::new();
        let mut parts = vec![0usize; kids.len()];
        for fragment in plan {
            parts[fragment.kid] += 1;
        }
        for fragment in plan {
            let kid = &kids[fragment.kid];
            let Some(measure) = self.children[fragment.kid].get_mut(fragment.copy) else {
                continue;
            };
            let clone = kid.clone_dec.is_some();
            let full_height = if clone { fragment.h } else { kid.h };
            let from = if clone { 0.0 } else { fragment.from };
            let available = size(
                AvailableSpace::Definite(px(col_width)),
                AvailableSpace::Definite(px(full_height)),
            );
            let clipped = parts[fragment.kid] > 1 || clone;
            let (_, first, last) = if clipped {
                measure.measure_slice(available, px(from), px(fragment.h), window, cx)
            } else {
                measure.measure(available, window, cx)
            };
            let line_y = match probe.rows {
                Some(rows) if rows.wrap => lines
                    .get(fragment.col / probe.count)
                    .map_or(0.0, |line| line.0),
                _ => 0.0,
            };
            let column = columns.entry(fragment.col).or_default();
            let y = line_y + fragment.y + self.relative_y[fragment.kid];
            let (head, foot) =
                self.repeated[fragment.kid].measure(fragment, available, y, window, cx);
            column.include_unclipped(head.first, head.last, 0.0, 0.0);
            if clipped {
                column.include(
                    first.map(f32::from),
                    last.map(f32::from),
                    from,
                    fragment.h,
                    y,
                );
            } else {
                column.include_unclipped(first.map(f32::from), last.map(f32::from), from, y);
            }
            column.include_unclipped(foot.first, foot.last, 0.0, 0.0);
        }
        let mut result = Baselines::default();
        for column in columns.values() {
            result.encompass(*column);
        }
        for &(kid, y) in spans {
            let (_, first, last) = self.children[kid][0].measure(
                size(
                    AvailableSpace::Definite(px(width)),
                    AvailableSpace::Definite(px(kids[kid].h)),
                ),
                window,
                cx,
            );
            result.encompass(Baselines {
                first: first.map(|b| y + f32::from(b)),
                last: last.map(|b| y + f32::from(b)),
            });
        }
        (result.first.map(px), result.last.map(px))
    }
}

#[derive(Clone, Copy, Default)]
struct Baselines {
    first: Option<f32>,
    last: Option<f32>,
}

impl Baselines {
    fn include(&mut self, first: Option<f32>, last: Option<f32>, from: f32, height: f32, y: f32) {
        // Match the paint mask only for a fragment that actually clips its content.
        let visible = |baseline: f32| baseline >= from && baseline <= from + height;
        let first = first.filter(|&baseline| visible(baseline));
        let last = last.filter(|&baseline| visible(baseline));
        self.include_unclipped(first, last, from, y);
    }

    fn include_unclipped(&mut self, first: Option<f32>, last: Option<f32>, from: f32, y: f32) {
        // Choose content in flow order inside each column; do not reorder children
        // by the position of their ink. Whole boxes retain visible overflow baselines.
        if self.first.is_none() {
            self.first = first.or(last).map(|baseline| y + baseline - from);
        }
        if let Some(baseline) = last.or(first) {
            self.last = Some(y + baseline - from);
        }
    }

    fn encompass(&mut self, other: Self) {
        if let Some(first) = other.first {
            self.first = Some(self.first.map_or(first, |existing| existing.min(first)));
        }
        if let Some(last) = other.last {
            self.last = Some(self.last.map_or(last, |existing| existing.max(last)));
        }
    }
}

#[cfg(test)]
#[path = "column_baselines_tests.rs"]
mod tests;

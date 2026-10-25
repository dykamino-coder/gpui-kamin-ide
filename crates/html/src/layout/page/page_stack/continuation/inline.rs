//! Продолжение корневых IFC отдельно от независимых потоков flex-элементов.
use crate::layout::fragment::flow::inline::Inline;
use crate::layout::fragment::flow::{ContentLine, FlowMeasure};
use crate::tolerance::COORD_EPS;

pub(super) fn resume(flow: &FlowMeasure, old: &[Inline], from: f32) -> Vec<ContentLine> {
    let lines: Vec<_> = old.iter().flat_map(|p| p.lines.iter().cloned()).collect();
    flow.resume_lines(&lines, from);
    old.iter()
        .filter_map(|p| p.lines.iter().find(|l| l.bottom > from + COORD_EPS))
        .cloned()
        .collect()
}

pub(super) fn offset(addresses: &[ContentLine], fresh: &[Inline]) -> Option<f32> {
    addresses
        .iter()
        .filter_map(|address| {
            fresh
                .iter()
                .flat_map(|p| &p.lines)
                .find(|l| {
                    l.paragraph == address.paragraph && l.state.text_pos == address.state.text_pos
                })
                .map(|l| l.top)
        })
        .min_by(f32::total_cmp)
}

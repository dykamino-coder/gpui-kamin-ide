//! Адреса IFC и float продолжаются в своих областях внутри нового fragmentainer.
use crate::layout::fragment::flow::inline::Inline;
use crate::layout::fragment::flow::{ContentLine, FlowMeasure};
use crate::tolerance::COORD_EPS;

pub(super) fn resume(
    flow: &FlowMeasure,
    old: &crate::layout::fragment::flow::inline::Geometry,
    from: f32,
) -> Vec<(Vec<usize>, ContentLine)> {
    flow.resume_paragraphs(&old.paragraphs, from);
    flow.resume_floats(&old.floats, &old.paragraphs, from);
    old.paragraphs
        .iter()
        .filter_map(|p| {
            p.lines
                .iter()
                .find(|l| l.bottom > from + COORD_EPS)
                .map(|l| (p.scope.clone(), l.clone()))
        })
        .collect()
}

pub(super) fn offset(addresses: &[(Vec<usize>, ContentLine)], fresh: &[Inline]) -> Option<f32> {
    addresses
        .iter()
        .filter_map(|(scope, address)| {
            fresh
                .iter()
                .filter(|p| &p.scope == scope)
                .flat_map(|p| &p.lines)
                .find(|l| {
                    l.paragraph == address.paragraph && l.state.text_pos == address.state.text_pos
                })
                .map(|l| l.top)
        })
        .min_by(f32::total_cmp)
}

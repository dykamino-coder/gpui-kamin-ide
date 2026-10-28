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

/// A break inside a line box only slices a line taller than the fragmentainer
/// (css-break-3 §4.4 monolithic breaking): the continuation keeps the sliced
/// distance into that line.
pub(super) fn offset(
    addresses: &[(Vec<usize>, ContentLine)],
    fresh: &[Inline],
    from: f32,
) -> Option<f32> {
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
                .map(|l| l.top + (from - address.top).max(0.0))
        })
        .min_by(f32::total_cmp)
}

pub(super) fn float_offset(
    old: &crate::layout::fragment::flow::inline::Geometry,
    fresh: &crate::layout::fragment::flow::inline::Geometry,
    from: f32,
) -> Option<f32> {
    old.floats
        .iter()
        .filter_map(|snapshot| {
            let next = fresh.floats.iter().find(|f| f.scope == snapshot.scope)?;
            crate::layout::fragment::flow::float_resume::Cursor::at(snapshot, &old.paragraphs, from)
                .offset(next, &fresh.paragraphs)
        })
        .min_by(f32::total_cmp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sliced_tall_line_keeps_the_distance_into_it() {
        let line = |top, bottom| ContentLine {
            top,
            bottom,
            ..Default::default()
        };
        let fresh = vec![Inline {
            lines: vec![line(4., 676.)],
            ..Default::default()
        }];
        let addresses = vec![(Vec::new(), line(0., 672.))];
        assert_eq!(offset(&addresses, &fresh, 192.), Some(196.));
        // A break at the line's top keeps the line's own position.
        assert_eq!(offset(&addresses, &fresh, 0.), Some(4.));
    }
}

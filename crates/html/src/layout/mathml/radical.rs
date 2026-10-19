//! MathML Core radical boxes: base, degree, stretched surd and vinculum.
use super::{
    arrangement::Rule,
    geometry::{Metrics, row},
};
#[derive(Clone, Copy)]
pub(super) struct Parameters {
    pub gap: f32,
    pub rule: f32,
    pub extra: f32,
    pub before: f32,
    pub after: f32,
    pub raise: f32,
    pub indexed: bool,
}
pub(super) fn base(children: &[Metrics], p: Parameters, rtl: bool) -> Metrics {
    if p.indexed {
        children.first().copied().unwrap_or_default()
    } else {
        row(children, rtl).0
    }
}
// Radical metrics, child offsets and the rule geometry form one layout result.
#[allow(clippy::type_complexity)]
pub(super) fn layout(
    children: &[Metrics],
    surd: Metrics,
    p: Parameters,
    rtl: bool,
) -> (Metrics, Vec<(f32, f32)>, Rule, (f32, f32)) {
    let base = base(children, p, rtl);
    let degree = if p.indexed {
        children.get(1).copied()
    } else {
        None
    };
    let degree_width = degree.map_or(0.0, |v| v.width);
    let before = if degree.is_some() {
        p.before.max(0.0)
    } else {
        0.0
    };
    let after = if degree.is_some() {
        p.after.max(-degree_width)
    } else {
        0.0
    };
    let operator_x = before + degree_width + after;
    let base_x = operator_x + surd.width;
    let height = surd.ascent + surd.descent;
    let radical_ascent = base.ascent + p.gap + p.rule + p.extra;
    let mut ascent = radical_ascent;
    let mut descent = base.descent.max(height + p.extra - ascent);
    let raise = p.raise * height;
    if let Some(index) = degree {
        ascent = ascent.max(-descent + raise + index.ascent + index.descent);
        descent = descent.max(descent - raise + index.ascent + index.descent);
    }
    let width = base_x + base.width;
    let mut offsets = if p.indexed {
        vec![(base_x, ascent - base.ascent)]
    } else {
        row(children, rtl)
            .1
            .into_iter()
            .map(|(x, y)| (x + base_x, y + ascent - base.ascent))
            .collect()
    };
    if let Some(index) = degree {
        offsets.push((
            before,
            ascent + descent - raise - index.ascent - index.descent,
        ));
    }
    let mut operator = (operator_x, ascent - radical_ascent + p.extra);
    let mut rule_x = base_x;
    if rtl {
        for (offset, child) in offsets.iter_mut().zip(children) {
            offset.0 = width - offset.0 - child.width;
        }
        operator.0 = width - operator.0 - surd.width;
        rule_x = width - base_x - base.width;
    }
    (
        Metrics {
            width,
            ascent,
            descent,
        },
        offsets,
        Some((
            rule_x,
            ascent - radical_ascent + p.extra,
            base.width,
            p.rule,
        )),
        operator,
    )
}

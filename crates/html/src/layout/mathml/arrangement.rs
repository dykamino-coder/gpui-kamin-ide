//! Передаёт измеренные дочерние боксы подходящему алгоритму MathML Core.
use super::{
    fraction,
    geometry::{Metrics, row},
    limits, multiscripts, padded,
    parameters::{Kind, ScriptKind},
};
pub(super) type Rule = Option<(f32, f32, f32, f32)>;
pub(super) fn arrange(
    children: &[Metrics],
    kind: Kind,
    rtl: bool,
    width: Option<f32>,
    height: Option<f32>,
) -> (Metrics, Vec<(f32, f32)>, Rule) {
    let (mut metrics, mut offsets, rule) = match kind {
        Kind::Fraction(p) if children.len() == 2 => {
            let l = fraction::fraction(children[0], children[1], p, width);
            (l.metrics, l.offsets.into(), l.rule)
        }
        Kind::Scripts(p, kind) if !children.is_empty() => {
            let (m, o) = multiscripts::layout(children, p, kind, rtl);
            (m, o, None)
        }
        Kind::Limits(p, kind) if children.len() >= 2 => {
            let under = matches!(kind, ScriptKind::Sub | ScriptKind::Both).then(|| children[1]);
            let over = match kind {
                ScriptKind::Sup => Some(children[1]),
                ScriptKind::Both => children.get(2).copied(),
                _ => None,
            };
            let (m, o) = limits::limits(children[0], under, over, p, width);
            (m, o, None)
        }
        _ => {
            let (m, o) = row(children, rtl);
            (m, o, None)
        }
    };
    if let Kind::Operator(left, right) = kind {
        for (x, _) in &mut offsets {
            *x += if rtl { right } else { left };
        }
        metrics.width += left + right;
    }
    if let Kind::Padded(p) = kind {
        let (outer, shift) = padded::padded(metrics, p);
        for (x, y) in &mut offsets {
            *x += shift[0];
            *y += shift[1];
        }
        metrics = outer;
    } else {
        if let Some(width) = width {
            if rtl && matches!(kind, Kind::Row | Kind::Scripts(..)) {
                for (x, _) in &mut offsets {
                    *x += width - metrics.width;
                }
            }
            metrics.width = width.max(0.0);
        }
        if let Some(height) = height {
            metrics.descent = height.max(0.0) - metrics.ascent;
        }
    }
    (metrics, offsets, rule)
}

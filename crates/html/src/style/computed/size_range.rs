//! Size ranges apply to a complete math expression, not its individual terms.
//! CSS Values 4 §10.12 accepts out-of-range math and clamps its resolved result.

use crate::style::values::value::Len;

pub(super) fn parse(value: &str) -> Option<Len> {
    let parsed = Len::parse_mixed(value)?;
    let lower = value.trim_start().to_ascii_lowercase();
    let math = ["calc(", "min(", "max(", "clamp("]
        .iter()
        .any(|function| lower.starts_with(function));
    match parsed {
        Len::Px(n) => scalar(n, math).map(Len::Px),
        Len::Pct(n) => scalar(n, math).map(Len::Pct),
        Len::Em(n) => scalar(n, math).map(Len::Em),
        Len::Vh(n) => scalar(n, math).map(Len::Vh),
        Len::Vw(n) => scalar(n, math).map(Len::Vw),
        Len::Ch(n) => scalar(n, math).map(Len::Ch),
        Len::Ex(n) => scalar(n, math).map(Len::Ex),
        Len::Ic(n) => scalar(n, math).map(Len::Ic),
        Len::Lh(n) => scalar(n, math).map(Len::Lh),
        // These terms have different bases. Their signs say nothing about the
        // combined size; resolve the font metrics before clamping at layout.
        Len::EmPx(_, _) | Len::LhPx(_, _) => math.then_some(parsed),
        _ => Some(parsed),
    }
}

fn scalar(value: f32, math: bool) -> Option<f32> {
    if math {
        Some(value.max(0.0))
    } else {
        (value >= 0.0).then_some(value)
    }
}

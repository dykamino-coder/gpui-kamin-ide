//! SVG number and transform syntax shared with the vendored SVG renderer.

use std::str::FromStr;

pub(super) fn valid_box(value: &str) -> bool {
    let nums: Result<Vec<_>, _> = svgtypes::NumberListParser::from(value).collect();
    nums.is_ok_and(|n| {
        n.len() == 4 && n.iter().all(|v| v.is_finite()) && n[2] >= 0.0 && n[3] >= 0.0
    })
}

pub(super) fn valid_transform(value: &str) -> bool {
    // svgtypes deliberately accepts dangling separators in attribute syntax;
    // a view specification must be valid in its entirety (SVG 2 §16.3.2).
    let compact: String = value.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    if compact.ends_with(',') || [",)", "(,", ",,"].iter().any(|s| compact.contains(s)) {
        return false;
    }
    svgtypes::Transform::from_str(value)
        .is_ok_and(|t| [t.a, t.b, t.c, t.d, t.e, t.f].iter().all(|n| n.is_finite()))
}

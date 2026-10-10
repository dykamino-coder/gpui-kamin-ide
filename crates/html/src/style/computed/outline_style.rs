//! Outline styles retain double so paint can leave the gap transparent.
//! CSS UI 4 §outline-style uses the corresponding border line patterns.

pub(crate) const DOUBLE: u8 = 5;

pub(super) fn parse(value: &str) -> Option<u8> {
    match value {
        "none" | "hidden" => Some(0),
        "auto" => Some(2),
        "dotted" => Some(3),
        "dashed" => Some(4),
        "double" => Some(DOUBLE),
        "solid" | "groove" | "ridge" | "inset" | "outset" => Some(1),
        _ => None,
    }
}

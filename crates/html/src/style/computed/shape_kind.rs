//! Linear versus curved contours in CSS border-shape values.

/// Контур `border-shape` состоит только из прямых (polygon, `path()`/`shape()`
/// без кривых)? У таких Blink держит предел митры 1e10 — острые углы уходят
/// шипами; у кривых — 4.0 по умолчанию (`border_shape_painter.cc`).
pub fn shape_is_linear(raw: &str) -> bool {
    let raw = raw.trim();
    if raw.starts_with("polygon(") {
        return true;
    }
    if let Some(d) = raw.strip_prefix("path(") {
        return !d.contains(|c: char| {
            matches!(c, 'C' | 'c' | 'S' | 's' | 'Q' | 'q' | 'T' | 't' | 'A' | 'a')
        });
    }
    if raw.starts_with("shape(") {
        return !(raw.contains("arc") || raw.contains("curve") || raw.contains("smooth"));
    }
    false
}

//! Media Fragments §6.1.2: spatial clipping establishes the SVG initial viewBox.
//! Percent coordinates use CSS Images' concrete size with the 300×150 default;
//! pixel clips are bounded only when the document supplies a natural size.

use crate::paint::background::{default_size, image_decode::svg_size};

pub(super) fn view_box(markup: &str, fragment: &str) -> Option<String> {
    // Media Fragments §6.3: the last syntactically valid occurrence wins.
    let (mut rect, mut percent) = (None, false);
    for part in fragment.split('&').filter_map(|p| p.strip_prefix("xywh=")) {
        if let Some((value, unit)) = parse(part) {
            rect = Some(value);
            percent = unit;
        }
    }
    let [mut x, mut y, mut w, mut h] = rect?;
    let intrinsic = svg_size(markup);
    if percent {
        let (width, height) = default_size(intrinsic, (300.0, 150.0));
        x *= width / 100.0;
        w *= width / 100.0;
        y *= height / 100.0;
        h *= height / 100.0;
    } else {
        let (width, height) = default_size(intrinsic, (0.0, 0.0));
        if width > 0.0 && height > 0.0 {
            w = w.min(width - x);
            h = h.min(height - y);
        }
    }
    (w > 0.0 && h > 0.0).then(|| format!("{x} {y} {w} {h}"))
}

fn parse(value: &str) -> Option<([f32; 4], bool)> {
    let (value, percent) = if let Some(value) = value.strip_prefix("percent:") {
        (value, true)
    } else {
        (value.strip_prefix("pixel:").unwrap_or(value), false)
    };
    let mut parts = value.split(',');
    let mut rect = [0.0; 4];
    for coordinate in &mut rect {
        let part = parts.next()?;
        // The spatial grammar uses unsigned integers, including percent units.
        if part.is_empty() || !part.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        *coordinate = part.parse::<f32>().ok().filter(|v| v.is_finite())?;
    }
    if parts.next().is_some() || rect[2] <= 0.0 || rect[3] <= 0.0 {
        return None;
    }
    if percent && (rect[0] + rect[2] > 100.0 || rect[1] + rect[3] > 100.0) {
        return None;
    }
    Some((rect, percent))
}

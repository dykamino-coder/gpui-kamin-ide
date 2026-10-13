//! `line-height: normal` in pixels with whole-pixel font metrics, as Blink computes it.
//!
//! Blink rounds the primary font's ascent, descent and line gap separately
//! (`SimpleFontData::PlatformInit`, `FontMetrics::AscentDescentWithHacks`)
//! and sums them into the line spacing.

use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    /// Normal line fraction (bits) -> ascent, descent and line gap fractions.
    static PARTS: RefCell<HashMap<u32, (f32, f32, f32)>> = RefCell::new(HashMap::new());
}

pub(super) fn register(line: f32, ascent: f32, descent: f32) {
    if ascent <= 0.0 || descent < 0.0 {
        return;
    }
    let gap = (line - ascent - descent).max(0.0);
    PARTS.with(|p| {
        p.borrow_mut()
            .insert(line.to_bits(), (ascent, descent, gap))
    });
}

pub fn normal_px(fraction: f32, size: f32) -> f32 {
    let parts = PARTS.with(|p| p.borrow().get(&fraction.to_bits()).copied());
    match parts {
        Some((a, d, g)) if size > 0.0 => {
            (a * size).round() + (d * size).round() + (g * size).round()
        }
        _ => fraction * size,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_each_metric_separately() {
        // Times New Roman: 1825 + 443 + 87 units of 2048.
        let (a, d, g) = (1825.0 / 2048.0, 443.0 / 2048.0, 87.0 / 2048.0);
        let line = a + d + g;
        register(line, a, d);
        assert_eq!(normal_px(line, 40.0), 47.0);
        assert_eq!(normal_px(line, 16.0), 18.0);
        assert_eq!(normal_px(1.5, 10.0), 15.0);
    }
}

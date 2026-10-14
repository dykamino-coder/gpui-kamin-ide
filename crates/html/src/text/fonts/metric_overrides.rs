//! CSS Fonts 4 metric descriptors, kept per CSS face rather than native family.
//!
//! Private font feature tags carry the selected face's metrics through GPUI's
//! font aliases. They do not change glyph shaping or the native font's metrics.

use std::cell::RefCell;
use std::collections::HashMap;

const TAGS: [&str; 3] = ["kasc", "kdsc", "kgap"];
type Overrides = [Option<f32>; 3];

thread_local! {
    static FACES: RefCell<HashMap<String, Vec<(u8, Overrides)>>> = RefCell::new(HashMap::new());
}

pub(super) fn clear() {
    FACES.with(|faces| faces.borrow_mut().clear());
}

fn percentage(value: &str) -> Option<f32> {
    let value = value.trim().strip_suffix('%')?.trim().parse::<f32>().ok()?;
    (value.is_finite() && value >= 0.0).then_some(value / 100.0)
}

pub(super) fn register(family: &str, slope: u8, block: &str) {
    let mut values = [None; 3];
    const NAMES: [&str; 3] = ["ascent-override", "descent-override", "line-gap-override"];
    for declaration in block.split(';') {
        let Some((name, value)) = declaration.split_once(':') else {
            continue;
        };
        let Some(index) = NAMES
            .iter()
            .position(|n| name.trim().eq_ignore_ascii_case(n))
        else {
            continue;
        };
        if value.trim().eq_ignore_ascii_case("normal") {
            values[index] = None;
        } else if let Some(value) = percentage(value) {
            values[index] = Some(value);
        }
    }
    FACES.with(|faces| {
        faces
            .borrow_mut()
            .entry(family.to_string())
            .or_default()
            .push((slope, values));
    });
}

fn selected(family: &str, slope: u8) -> Overrides {
    FACES.with(|faces| {
        let faces = faces.borrow();
        let Some(faces) = faces.get(&family.to_ascii_lowercase()) else {
            return [None; 3];
        };
        let order = match slope {
            1 => [1, 2, 0],
            2 => [2, 0, 1],
            _ => [0, 2, 1],
        };
        for slope in order {
            let mut found = faces.iter().filter(|(s, _)| *s == slope).map(|(_, v)| *v);
            if let Some(first) = found.next() {
                return if found.all(|v| v == first) {
                    first
                } else {
                    [None; 3]
                };
            }
        }
        [None; 3]
    })
}

pub(crate) fn features(family: &str, slope: u8) -> Vec<(String, u32)> {
    TAGS.into_iter()
        .zip(selected(family, slope))
        .filter_map(|(tag, value)| value.map(|v| (tag.to_string(), v.to_bits())))
        .collect()
}

pub(crate) fn has_overrides(font: &gpui::Font) -> bool {
    font.features
        .tag_value_list()
        .iter()
        .any(|(tag, _)| TAGS.contains(&tag.as_str()))
}

/// Override ascent/descent in used pixels. Line gap belongs to line-height,
/// not to either extent: half-leading distributes it around the baseline.
pub(crate) fn extents(font: &gpui::Font, size: f32, native: (f32, f32)) -> (f32, f32) {
    let mut result = [native.0, native.1];
    for (tag, value) in font.features.tag_value_list() {
        if let Some(index) = TAGS[..2].iter().position(|t| *t == tag) {
            result[index] = f32::from_bits(*value) * size;
        }
    }
    (result[0], result[1])
}

pub(crate) fn normal_line(family: &str, native: f32) -> f32 {
    let [ascent, descent, gap] = selected(family, 0);
    if ascent.is_none() && descent.is_none() && gap.is_none() {
        return native;
    }
    let (a, d, _) = crate::text::metrics::vmetrics_px(family, 1.0);
    ascent.unwrap_or(a) + descent.unwrap_or(d) + gap.unwrap_or((native - a - d).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metric_percentages_reject_negative_nonfinite_and_nonpercentage_values() {
        assert_eq!(percentage("125%"), Some(1.25));
        assert_eq!(percentage("0%"), Some(0.0));
        for invalid in ["normal", "-1%", "NaN%", "inf%", "20px", "10% 20%"] {
            assert_eq!(percentage(invalid), None, "{invalid}");
        }
    }

    #[test]
    fn last_valid_descriptor_wins_and_normal_restores_native_metrics() {
        clear();
        register(
            "face",
            0,
            "ascent-override:50%;ascent-override:normal;descent-override:20%;descent-override:-10%;line-gap-override:10%;LINE-GAP-OVERRIDE:30%",
        );
        assert_eq!(selected("face", 0), [None, Some(0.2), Some(0.3)]);
        clear();
    }

    #[test]
    fn css_aliases_keep_independent_extents_and_leave_glyph_metrics_alone() {
        clear();
        register(
            "overridden",
            0,
            "ascent-override:100%;descent-override:50%;line-gap-override:100%",
        );
        let mut font = gpui::font("Ahem");
        font.features = gpui::FontFeatures(std::sync::Arc::new(features("overridden", 0)));
        assert_eq!(extents(&font, 40.0, (32.0, 8.0)), (40.0, 20.0));
        assert_eq!(normal_line("overridden", 1.0), 2.5);
        assert_eq!(normal_line("Ahem", 1.0), 1.0);
        assert!(features("Ahem", 0).is_empty());
        clear();
    }
}

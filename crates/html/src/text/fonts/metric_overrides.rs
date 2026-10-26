//! Дескрипторы метрик `@font-face` (css-fonts-4): `ascent-override`, `descent-override`,
//! `line-gap-override`, хранимые на CSS-лицо, а не на системное семейство. Приватные теги
//! возможностей (`kasc`, `kdsc`, `kgap`) проносят метрики выбранного лица через алиасы шрифтов
//! gpui; они не меняют ни шейпинг глифов, ни метрики системного шрифта.

use crate::document::state::ValueCx;

const TAGS: [&str; 3] = ["kasc", "kdsc", "kgap"];
type Overrides = [Option<f32>; 3];

pub(crate) use crate::font_facts::MetricFaces;

pub(crate) fn clear(vcx: ValueCx<'_>) {
    vcx.doc.font_metric_faces.borrow_mut().clear();
    vcx.doc.font_registry_changed();
}

fn percentage(value: &str) -> Option<f32> {
    let value = value.trim().strip_suffix('%')?.trim().parse::<f32>().ok()?;
    (value.is_finite() && value >= 0.0).then_some(value / 100.0)
}

pub(crate) fn register(family: &str, slope: u8, block: &str, vcx: ValueCx<'_>) {
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
    {
        vcx.doc
            .font_metric_faces
            .borrow_mut()
            .entry(family.to_string())
            .or_default()
            .push((slope, values));
    }
    vcx.doc.font_registry_changed();
}

fn selected(family: &str, slope: u8, vcx: ValueCx<'_>) -> Overrides {
    vcx.doc
        .font_metric_faces
        .borrow()
        .get(&family.to_ascii_lowercase())
        .and_then(|faces| crate::font_facts::uniform_slope(faces, slope))
        .unwrap_or([None; 3])
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
    let mut overrides = [None; 3];
    for (tag, value) in font.features.tag_value_list() {
        if let Some(index) = TAGS[..2].iter().position(|t| *t == tag) {
            overrides[index] = Some(f32::from_bits(*value));
        }
    }
    let result =
        crate::font_facts::UsedFont::override_metrics(size, [native.0, native.1, 0.0], overrides);
    (result[0], result[1])
}

pub(crate) fn normal_line(family: &str, native: f32, vcx: ValueCx<'_>) -> f32 {
    let [ascent, descent, gap] = selected(family, 0, vcx);
    if ascent.is_none() && descent.is_none() && gap.is_none() {
        return native;
    }
    let (a, d, _) = crate::text::metrics::vmetrics_px(family, 1.0, vcx);
    crate::font_facts::UsedFont::override_metrics(
        1.0,
        [a, d, (native - a - d).max(0.0)],
        [ascent, descent, gap],
    )
    .into_iter()
    .sum()
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
        let doc = crate::document::state::DocState::new(Default::default());
        let vcx = doc.values();
        clear(vcx);
        register(
            "face",
            0,
            "ascent-override:50%;ascent-override:normal;descent-override:20%;descent-override:-10%;line-gap-override:10%;LINE-GAP-OVERRIDE:30%",
            vcx,
        );
        assert_eq!(selected("face", 0, vcx), [None, Some(0.2), Some(0.3)]);
        clear(vcx);
    }

    #[test]
    fn css_aliases_keep_independent_extents_and_leave_glyph_metrics_alone() {
        let doc = crate::document::state::DocState::new(Default::default());
        let vcx = doc.values();
        clear(vcx);
        register(
            "overridden",
            0,
            "ascent-override:100%;descent-override:50%;line-gap-override:100%",
            vcx,
        );
        let mut font = gpui::font("Ahem");
        font.features = gpui::FontFeatures(std::sync::Arc::new(
            vcx.doc.font_snapshot().metric_features("overridden", 0),
        ));
        assert_eq!(extents(&font, 40.0, (32.0, 8.0)), (40.0, 20.0));
        assert_eq!(normal_line("overridden", 1.0, vcx), 2.5);
        assert_eq!(normal_line("Ahem", 1.0, vcx), 1.0);
        assert!(
            vcx.doc
                .font_snapshot()
                .metric_features("Ahem", 0)
                .is_empty()
        );
        clear(vcx);
    }
    #[test]
    fn same_family_descriptors_stay_with_their_document() {
        let a = crate::document::state::DocState::new(Default::default());
        let b = crate::document::state::DocState::new(Default::default());
        register("shared", 0, "ascent-override:50%", a.values());
        register("shared", 0, "ascent-override:150%", b.values());
        assert_eq!(selected("shared", 0, a.values())[0], Some(0.5));
        assert_eq!(selected("shared", 0, b.values())[0], Some(1.5));
        clear(b.values());
        assert_eq!(selected("shared", 0, a.values())[0], Some(0.5));
    }
}

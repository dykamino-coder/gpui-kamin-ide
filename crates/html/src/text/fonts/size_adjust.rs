//! Native fallback contract: the target raw metric relative to the run's used em.

use crate::{style::computed::Computed, values::ValueCx};

pub(crate) fn apply(style: &Computed, font: &mut gpui::Font, vcx: ValueCx<'_>) {
    font.fallbacks = match target(style, vcx) {
        Some(adjustment) => Some(
            font.fallbacks
                .take()
                .unwrap_or_default()
                .with_size_adjustment(adjustment),
        ),
        None => font
            .fallbacks
            .take()
            .map(gpui::FontFallbacks::without_size_adjustment),
    };
}

fn target(style: &Computed, vcx: ValueCx<'_>) -> Option<gpui::FontSizeAdjustment> {
    let (metric, want) = style.font_adjustment()?;
    let used = style.used_font(1.0, vcx);
    // The scale is independent of computed font-size, including a zero em.
    let scale = used.descriptor_scale * used.property_scale;
    let aspect = want / scale;
    if !aspect.is_finite() || aspect <= 0.0 {
        return None;
    }
    use gpui::FontSizeMetric::*;
    let metric = match metric {
        0 => XHeight,
        1 => CapHeight,
        2 => ZeroAdvance,
        3 => IdeographicWidth,
        4 => IdeographicHeight,
        _ => return None,
    };
    Some(gpui::FontSizeAdjustment { metric, aspect })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::values::value::Len;

    #[test]
    fn fallback_adjustment_is_relative_to_used_em_and_not_computed_size() {
        let doc = crate::document::state::DocState::new(Default::default());
        let mut style = Computed::default();
        style.inh.font.font_size_adjust = Some((0, 0.75));
        let mut adjustment = None;
        for size in [0.0, 16.0, 32.0] {
            style.inh.font.font_size = Some(Len::Px(size));
            let mut font = gpui::font("test");
            apply(&style, &mut font, doc.values());
            let actual = font.fallbacks.unwrap().size_adjustment();
            assert_eq!(actual.unwrap().metric, gpui::FontSizeMetric::XHeight);
            assert_eq!(actual.unwrap().aspect, 0.5);
            if let Some(previous) = adjustment {
                assert_eq!(actual, previous);
            }
            adjustment = Some(actual);
        }
    }

    #[test]
    fn none_does_not_allocate_a_fallback_configuration() {
        let doc = crate::document::state::DocState::new(Default::default());
        let mut font = gpui::font("test");
        apply(&Computed::default(), &mut font, doc.values());
        assert!(font.fallbacks.is_none());
    }

    #[test]
    fn none_resets_the_paragraphs_adjustment_but_preserves_its_families() {
        let doc = crate::document::state::DocState::new(Default::default());
        let mut font = gpui::font("test");
        font.fallbacks = Some(
            gpui::FontFallbacks::from_fonts(vec!["fallback".into()]).with_size_adjustment(
                gpui::FontSizeAdjustment {
                    metric: gpui::FontSizeMetric::XHeight,
                    aspect: 0.75,
                },
            ),
        );
        apply(&Computed::default(), &mut font, doc.values());
        let fallback = font.fallbacks.unwrap();
        assert_eq!(fallback.size_adjustment(), None);
        assert_eq!(fallback.fallback_list(), ["fallback"]);
    }
}

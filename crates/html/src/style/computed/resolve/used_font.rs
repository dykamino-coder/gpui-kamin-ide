//! Первичное лицо: computed размер отдельно от дескриптора и свойства подгонки.

use super::*;
use crate::font_facts::{FontMetricFacts, FontMetricQuery, UsedFont};

fn aspect(facts: &FontMetricFacts, metric: u8) -> Option<f32> {
    match metric {
        0 => facts.x_height,
        1 => facts.cap_height,
        2 => facts.zero_advance,
        3 => facts.ideographic_advance,
        4 => facts.ideographic_vertical_advance,
        _ => None,
    }
    .map(|m| m.px)
    .filter(|m| m.is_finite() && *m >= 0.0)
}

impl Computed {
    pub(super) fn font_aspect(&self, family: &str, metric: u8, vcx: ValueCx<'_>) -> Option<f32> {
        let mut request = self.font_request(family, 1.0);
        request.metric_query = FontMetricQuery::Adjustment;
        let facts = vcx
            .doc
            .font_provider()
            .metrics(request, &vcx.doc.font_snapshot())
            .ok()?;
        aspect(&facts, metric)
    }

    /// `from-font` публикуется числом; потомок наследует пропорцию предка.
    /// Без сырой метрики вычисляется none: https://drafts.csswg.org/css-fonts-5/#font-size-adjust-prop
    pub(crate) fn compute_font_adjust(&mut self, family: &str, vcx: ValueCx<'_>) {
        if let Some((metric, want)) = self.font_size_adjust
            && want.is_nan()
        {
            let descriptor = self.font_descriptor_scale(family, vcx);
            self.font_size_adjust = Some(
                self.font_aspect(family, metric, vcx)
                    .map_or((u8::MAX, 0.0), |ratio| (metric, ratio * descriptor)),
            );
        }
    }

    fn font_descriptor_scale(&self, family: &str, vcx: ValueCx<'_>) -> f32 {
        let snapshot = vcx.doc.font_snapshot();
        let slope = if self.oblique == Some(true) {
            2
        } else {
            u8::from(self.italic == Some(true))
        };
        snapshot
            .matching_face(family, self.font_request(family, 1.0))
            .map_or_else(|| snapshot.size_adjust(family, slope), |f| f.adjust)
    }

    /// Результат для first available face; не изменяет CSS-значения элемента.
    pub fn used_font(&self, fallback_px: f32, vcx: ValueCx<'_>) -> UsedFont {
        let family = match self.font_family.as_deref() {
            Some(f) => f,
            None if self.monospace == Some(true) => {
                vcx.doc.font_snapshot().mono_family(self.lang.as_deref())
            }
            None => "",
        };
        self.used_font_for(family, fallback_px, vcx)
    }

    pub(crate) fn used_font_for(
        &self,
        family: &str,
        fallback_px: f32,
        vcx: ValueCx<'_>,
    ) -> UsedFont {
        let computed_px = match self.font_size {
            Some(Len::Px(v)) => v,
            Some(Len::Em(k) | Len::Pct(k)) => k * fallback_px,
            _ => fallback_px,
        };
        let snapshot = vcx.doc.font_snapshot();
        let selected = vcx
            .doc
            .font_provider()
            .select(self.font_request(family, computed_px), &snapshot)
            .ok();
        let descriptor = self.font_descriptor_scale(family, vcx);
        let adjustment = self.font_size_adjust.and_then(|(metric, want)| {
            if !want.is_finite() {
                return None;
            }
            let have = self
                .font_aspect(family, metric, vcx)
                .or_else(|| match metric {
                    // https://github.com/w3c/csswg-drafts/issues/6384#issuecomment-2163284492
                    // Отсутствующая метрика использует fallback единицы
                    // (css-values-4 §6.1.1); from-font выше требует самой метрики.
                    0 | 2 => Some(0.5),
                    1 => {
                        let mut request = self.font_request(family, 1.0);
                        request.metric_query = FontMetricQuery::Adjustment;
                        vcx.doc
                            .font_provider()
                            .metrics(request, &snapshot)
                            .ok()?
                            .ascent
                            .map(|m| m.px)
                    }
                    3 | 4 => Some(1.0),
                    _ => None,
                });
            have.map(|have| (want, have * descriptor))
        });
        UsedFont::new(selected, computed_px, descriptor, adjustment)
    }

    /// Псевдоэлемент заменяет размер вместе с его независимой подгонкой.
    pub(crate) fn overlay_font_size(&mut self, pseudo: &Computed) {
        self.font_size = pseudo.font_size.or(self.font_size);
        self.font_size_adjust = pseudo.font_size_adjust.or(self.font_size_adjust);
    }

    /// Числовая высота строки относится к computed размеру, не к контурам.
    pub(crate) fn numeric_line_px(&self, fallback_px: f32) -> Option<f32> {
        self.numeric_line_value_px(self.line_height, fallback_px)
    }

    pub(crate) fn numeric_line_value_px(&self, line: Option<Len>, fallback_px: f32) -> Option<f32> {
        let computed = match self.font_size {
            Some(Len::Px(v)) => v,
            Some(Len::Em(k) | Len::Pct(k)) => k * fallback_px,
            _ => fallback_px,
        };
        match line {
            Some(Len::Px(v)) => Some(v),
            Some(Len::Pct(k) | Len::Em(k)) => Some(k * computed),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;

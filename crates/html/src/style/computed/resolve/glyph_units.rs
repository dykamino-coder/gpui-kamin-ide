//! Единицы от глифов первичного лица после подгонки, без изменения базы em.

use super::*;
use crate::font_facts::FontMetricQuery;

impl Computed {
    pub(crate) fn glyph_units(&self, family: &str, computed: f32, vcx: ValueCx<'_>) -> [f32; 4] {
        let used = self.used_font_for(family, computed, vcx).size_px;
        let (ch, ex) = crate::style::font_metrics::horizontal(family, used, vcx);
        let cap = crate::style::font_metrics::vertical(family, used, vcx).2;
        let ic = crate::style::font_metrics::ideographic(family, used, vcx);
        if self.font_size_adjust.is_none_or(|(metric, _)| metric >= 5) {
            return [cap, ch, ex, ic];
        }
        let mut request = self.font_request(family, used);
        request.metric_query = FontMetricQuery::Adjustment;
        let Ok(facts) = vcx
            .doc
            .font_provider()
            .metrics(request, &vcx.doc.font_snapshot())
        else {
            return [cap, ch, ex, ic];
        };
        [
            facts.cap_height.map_or(cap, |m| m.px),
            facts.zero_advance.map_or(ch, |m| m.px),
            facts.x_height.map_or(ex, |m| m.px),
            facts.ideographic_advance.map_or(ic, |m| m.px),
        ]
    }
}

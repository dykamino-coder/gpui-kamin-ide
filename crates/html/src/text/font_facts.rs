//! Адаптер прежних измерений text к нейтральным фактам документа.
//! Сохраняет порядок замеров, overrides и округление normal; не объявляет
//! разрешённые backend-значения сырыми метриками или результатом face matching.

use crate::document::state::DocState;
use crate::font_facts::{
    FontFactsError, FontFactsProvider, FontMetric, FontMetricFacts, FontMetricQuery,
    FontRegistrySnapshot, FontRequest, NormalLineFacts,
};
use std::rc::{Rc, Weak};

mod selection;

pub(crate) struct Backend {
    document: Weak<DocState>,
}

impl Backend {
    pub(crate) fn new(document: Weak<DocState>) -> Self {
        Self { document }
    }

    fn document(&self, snapshot: &FontRegistrySnapshot) -> Result<Rc<DocState>, FontFactsError> {
        let doc = self.document.upgrade().ok_or(FontFactsError::Unavailable)?;
        let current = doc.font_snapshot();
        if current.environment != snapshot.environment {
            return Err(FontFactsError::WrongEnvironment);
        }
        if current.generation != snapshot.generation {
            return Err(FontFactsError::StaleSnapshot);
        }
        Ok(doc)
    }
}

impl FontFactsProvider for Backend {
    fn select(
        &self,
        request: FontRequest<'_>,
        snapshot: &FontRegistrySnapshot,
    ) -> Result<crate::font_facts::SelectedFaceFacts, FontFactsError> {
        self.select_face(request, snapshot)
    }

    fn metrics(
        &self,
        request: FontRequest<'_>,
        snapshot: &FontRegistrySnapshot,
    ) -> Result<FontMetricFacts, FontFactsError> {
        let doc = self.document(snapshot)?;
        let vcx = doc.values();
        use FontMetricQuery::*;
        let mut facts = FontMetricFacts::default();
        let metric = |v| Some(FontMetric::backend_resolved(v));
        if matches!(request.metric_query, All | Horizontal) {
            let (ch, ex) = super::metrics::ch_ex_px(request.family_list, request.size_px, vcx);
            facts.zero_advance = metric(ch);
            facts.x_height = metric(ex);
        }
        if matches!(request.metric_query, All | Vertical) {
            let (a, d, cap) =
                super::metrics::vmetrics_px(request.family_list, request.size_px, vcx);
            facts.ascent = metric(a);
            facts.descent = metric(d);
            facts.cap_height = metric(cap);
        }
        if matches!(request.metric_query, All | Ideographic) {
            facts.ideographic_advance = metric(super::metrics::ic_px(
                request.family_list,
                request.size_px,
                vcx,
            ));
        }
        if request.metric_query == Math {
            let actual = snapshot
                .alias_for(request.family_list, request)
                .unwrap_or_else(|| request.family_list.to_string());
            facts.math_script_scale = super::fonts::math::for_family(&actual, vcx)
                .map(|v| (v.script_scale, v.script_script_scale));
        }
        if request.metric_query == Alphabetic {
            facts.alphabetic_baseline =
                metric(super::fonts::alphabetic_em(request.family_list, vcx) * request.size_px);
        }
        Ok(facts)
    }

    fn normal_line(
        &self,
        request: FontRequest<'_>,
        snapshot: &FontRegistrySnapshot,
    ) -> Result<NormalLineFacts, FontFactsError> {
        let doc = self.document(snapshot)?;
        let vcx = doc.values();
        let fraction = super::metrics::normal_line(request.family_list, vcx);
        Ok(NormalLineFacts {
            fraction: Some(fraction),
            height: Some(FontMetric::backend_resolved(super::metrics::normal_px(
                fraction,
                request.size_px,
                vcx,
            ))),
            content_height: super::metrics::content_px(fraction, request.size_px, vcx)
                .map(FontMetric::backend_resolved),
        })
    }
}

#[cfg(test)]
mod tests;

//! Адаптер прежних измерений text к нейтральным фактам документа.
//! Сохраняет порядок замеров, overrides и округление normal; не объявляет
//! разрешённые backend-значения сырыми метриками или результатом face matching.

use crate::document::state::DocState;
use crate::font_facts::{
    FontFactsError, FontFactsProvider, FontMetric, FontMetricFacts, FontRegistrySnapshot,
    FontRequest, NormalLineFacts,
};
use std::rc::{Rc, Weak};

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
    fn metrics(
        &self,
        request: FontRequest<'_>,
        snapshot: &FontRegistrySnapshot,
    ) -> Result<FontMetricFacts, FontFactsError> {
        let doc = self.document(snapshot)?;
        let vcx = doc.values();
        let (ch, ex) = super::metrics::ch_ex_px(request.family_list, request.size_px, vcx);
        let (ascent, _, cap) =
            super::metrics::vmetrics_px(request.family_list, request.size_px, vcx);
        let ic = super::metrics::ic_px(request.family_list, request.size_px, vcx);
        Ok(FontMetricFacts {
            ascent: FontMetric::backend_resolved(ascent),
            cap_height: Some(FontMetric::backend_resolved(cap)),
            x_height: Some(FontMetric::backend_resolved(ex)),
            zero_advance: Some(FontMetric::backend_resolved(ch)),
            ideographic_advance: Some(FontMetric::backend_resolved(ic)),
        })
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
            height: (fraction > 0.0).then(|| {
                FontMetric::backend_resolved(super::metrics::normal_px(
                    fraction,
                    request.size_px,
                    vcx,
                ))
            }),
        })
    }
}

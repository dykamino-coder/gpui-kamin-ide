//! Провайдер шрифтовых фактов документа без бэкенда text: каждое чтение
//! сообщает отсутствие фактов, и читатели CSS берут запасные значения
//! спецификаций. Им пользуются нижние данные, созданные без верхнего документа.

use crate::font_facts::{
    FontFactsError, FontFactsProvider, FontMetricFacts, FontRegistrySnapshot, FontRequest,
    NormalLineFacts,
};

pub(crate) struct Detached;

impl FontFactsProvider for Detached {
    fn metrics(
        &self,
        _: FontRequest<'_>,
        _: &FontRegistrySnapshot,
    ) -> Result<FontMetricFacts, FontFactsError> {
        Err(FontFactsError::Unavailable)
    }

    fn normal_line(
        &self,
        _: FontRequest<'_>,
        _: &FontRegistrySnapshot,
    ) -> Result<NormalLineFacts, FontFactsError> {
        Err(FontFactsError::Unavailable)
    }
}

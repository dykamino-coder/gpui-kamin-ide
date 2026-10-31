//! Провайдер шрифтовых фактов документа без бэкенда text: каждое чтение
//! сообщает отсутствие фактов, и читатели CSS берут запасные значения
//! спецификаций. Им пользуются нижние данные, созданные без верхнего документа.

use {
    kamin_font_facts::font_facts::FontFactsError, kamin_font_facts::font_facts::FontFactsProvider,
    kamin_font_facts::font_facts::FontMetricFacts,
    kamin_font_facts::font_facts::FontRegistrySnapshot, kamin_font_facts::font_facts::FontRequest,
    kamin_font_facts::font_facts::NormalLineFacts,
};

pub struct Detached;

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

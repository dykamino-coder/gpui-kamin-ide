//! Состояние документа целиком: нижние данные значений (`DocumentValues`) и
//! типизированные состояния фаз раскладки и текста этого документа. Владелец —
//! `Document` (или вложенный документ рамки); сборка кадра получает их через
//! `LayoutCx`, нижний контекст значений — только `DocumentValues`.

use crate::style::css::{Media, PageRule};
use crate::values::{DocumentValues, ValueCx};
use std::rc::Rc;

pub struct DocState {
    /// Нижние данные: арены, at-правила, метрики корня, ресурсы, шрифтовые факты.
    pub(crate) values: Rc<DocumentValues>,
    /// Anchor registries and root frame geometry of the layout phase of this document.
    pub(crate) layout: Rc<crate::layout::layout_state::LayoutState>,
    /// Platform font context, measurement caches and MATH tables of the text backend.
    pub(crate) text: Rc<crate::text::text_fonts::TextFonts>,
    /// Paragraph and vertical-text measurements of the text phase of this document.
    pub(crate) paragraph: crate::text::paragraph_state::ParagraphState,
}

impl DocState {
    /// Пустое состояние документа, разбираемого носителем `media`. Провайдер
    /// шрифтовых фактов по умолчанию — бэкенд text этого документа.
    pub fn new(media: Media) -> Rc<Self> {
        Self::with_paragraphs(media, Default::default())
    }

    /// Состояние следующей ревизии того же документа (`Document::update`): замеры
    /// абзацев (`MeasureCache`) переходят к ней. Их ключи — значения всех входов
    /// разреза и подбора (текст, шрифты, ширина, правила переноса), а не узлы или
    /// записи арен, поэтому попадание в новой ревизии точно так же, как в
    /// следующем кадре; кэш хранит только записи прошлого кадра.
    pub(crate) fn continuing(media: Media, previous: &Self) -> Rc<Self> {
        Self::with_paragraphs(
            media,
            crate::text::paragraph_state::ParagraphState::continuing(&previous.paragraph),
        )
    }

    fn with_paragraphs(
        media: Media,
        paragraph: crate::text::paragraph_state::ParagraphState,
    ) -> Rc<Self> {
        let text = Rc::<crate::text::text_fonts::TextFonts>::default();
        let fonts = text.clone();
        let values = DocumentValues::new(media, move |values| {
            Rc::new(crate::text::font_facts::Backend::new(values, fonts))
        });
        Rc::new(Self {
            values,
            layout: Default::default(),
            text,
            paragraph,
        })
    }

    /// Начало сборки кадра хостом, в прежнем порядке: готовность и картинки,
    /// замеры текста, затем сброс области кадра уже покрашенного кадра.
    pub(crate) fn begin_frame(&self) {
        self.values.begin_frame_inputs();
        self.paragraph.begin_frame();
        self.values.reset_painted_frame();
    }

    /// Контекст значений разбора и каскада документа.
    pub fn values(&self) -> ValueCx<'_> {
        self.values.values()
    }

    /// Контекст значений сборки кадра: записи арен живут до краски кадра.
    pub fn frame_values(&self) -> ValueCx<'_> {
        self.values.frame_values()
    }

    /// Правила `@page` документа (копией — их читает хост на каждом кадре).
    pub fn page_rules(&self) -> Vec<PageRule> {
        self.values.page_rules()
    }

    /// Носитель документа.
    pub fn media(&self) -> Media {
        self.values.media()
    }
}

//! Состояние документа целиком: нижние данные значений (`DocumentValues`) и
//! типизированные состояния фаз раскладки и текста этого документа. Владелец —
//! `Document` (или вложенный документ рамки); сборка кадра получает их через
//! `LayoutCx`, нижний контекст значений — только `DocumentValues`.

use crate::style::css::{Media, PageRule};
use crate::values::{DocumentValues, ValueCx};
use std::rc::Rc;

pub struct DocState {
    /// Shared frame time and diagnostics across revisions of this document.
    pub(crate) animation: Rc<crate::animation::clock::Clock>,
    pub(crate) scrolling: Rc<crate::interactive::scroll_animation::Tracker>,
    /// The full-page view's canonical scroll owner; absent for ordinary builds and print.
    pub(crate) root_scroll: Option<Rc<crate::document::viewport::RootScroll>>,
    /// The single mutable box tree of this line of document revisions.
    pub(crate) boxes: Rc<std::cell::RefCell<crate::document::boxes::BoxTree>>,
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
        Self::for_view(media, false)
    }

    pub(crate) fn for_view(media: Media, viewport: bool) -> Rc<Self> {
        Self::with_paragraphs(
            media,
            Default::default(),
            None,
            viewport.then(Default::default),
        )
    }

    /// Состояние следующей ревизии того же документа (`Document::update`): замеры
    /// абзацев (`MeasureCache`) переходят к ней. Их ключи — значения всех входов
    /// разреза и подбора (текст, шрифты, ширина, правила переноса), а не узлы или
    /// записи арен, поэтому попадание в новой ревизии точно так же, как в
    /// следующем кадре; кэш хранит только записи прошлого кадра. The initial
    /// style groups are the previous revision's (DocumentValues::continuing):
    /// unwritten groups of both cascades are then the same pointers, which
    /// the rebuild's restyle damage compares first (layout::reuse::Store::rebase).
    pub(crate) fn continuing(media: Media, previous: &Self) -> Rc<Self> {
        Self::with_paragraphs(
            media,
            crate::text::paragraph_state::ParagraphState::continuing(&previous.paragraph),
            Some(previous),
            previous.root_scroll.clone(),
        )
    }

    /// Ревизия той же формы без перерасчёта стиля (`Document::adopt`): тот же
    /// документ в смысле HTML, поэтому значения (арены каскадных стилей, шрифты,
    /// at-правила, картинки) и текстовый бэкенд остаются прежними, а факты
    /// прошлого дерева (раскладка прошлого кадра, размеры повёрнутых поддеревьев)
    /// начинаются заново, как у полной сборки; подготовка прошлого дерева —
    /// основа подготовки нового (`LayoutState::continuing`).
    pub(crate) fn adopting(previous: &Self) -> Rc<Self> {
        Rc::new(Self {
            root_scroll: previous.root_scroll.clone(),
            animation: previous.animation.clone(),
            scrolling: previous.scrolling.clone(),
            boxes: previous.boxes.clone(),
            values: previous.values.clone(),
            layout: Rc::new(crate::layout::layout_state::LayoutState::continuing(
                &previous.layout,
            )),
            text: previous.text.clone(),
            paragraph: crate::text::paragraph_state::ParagraphState::continuing(
                &previous.paragraph,
            ),
        })
    }

    fn with_paragraphs(
        media: Media,
        paragraph: crate::text::paragraph_state::ParagraphState,
        previous: Option<&Self>,
        root_scroll: Option<Rc<crate::document::viewport::RootScroll>>,
    ) -> Rc<Self> {
        let text = Rc::<crate::text::text_fonts::TextFonts>::default();
        let fonts = text.clone();
        let fonts = move |values: std::rc::Weak<DocumentValues>| -> Rc<
            dyn kamin_font_facts::font_facts::FontFactsProvider,
        > {
            Rc::new(crate::text::font_facts::Backend::new(values, fonts))
        };
        let values = match previous {
            Some(previous) => DocumentValues::continuing(&previous.values, media, fonts),
            None => DocumentValues::new(media, fonts),
        };
        Rc::new(Self {
            root_scroll,
            animation: previous.map(|p| p.animation.clone()).unwrap_or_default(),
            scrolling: previous.map(|p| p.scrolling.clone()).unwrap_or_default(),
            boxes: previous.map(|p| p.boxes.clone()).unwrap_or_default(),
            values,
            layout: Default::default(),
            text,
            paragraph,
        })
    }

    /// Начало сборки кадра хостом, в прежнем порядке: готовность и картинки,
    /// замеры текста, затем сброс области кадра уже покрашенного кадра.
    pub(crate) fn begin_frame(&self) {
        if let Some(root) = &self.root_scroll {
            // A current scroll adapter enables input during layout, including after restyles.
            root.axes.set([false; 2]);
        }
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

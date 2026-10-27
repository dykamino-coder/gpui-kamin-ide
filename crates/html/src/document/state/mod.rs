//! Состояние, принадлежащее одному документу: его at-правила (`@page`,
//! `@position-try`, `@property`, `@counter-style`, `@color-profile`),
//! метрики корня (`rem`/`rlh`), режим quirks и носитель (`Media`).
//!
//! Владелец — `Document` (или вложенный документ рамки, `Embedded`), а не
//! процесс: так устроены Blink (`StyleEngine`/`TreeScope` документа) и Stylo
//! (`Stylist` на документ). Документ передаётся явно: разбор листа и дерева
//! получает его параметром (`SheetCx`, `TreeIndex`), значения — контекстом
//! `ValueCx`, сборка кадра — `LayoutCx`, фазы GPUI — элементы, которые
//! держат его сами (`ValueDoc`).
//!
//! Арены `calc()`/`anchor()` документа и их ручки — `arena`.

mod arena;
mod font_facts;
mod property_registry;
mod readiness;
mod scoped;
mod synthetic_ids;
mod traversal;
pub(crate) use traversal::MAX_DEPTH;
mod value_cx;

use crate::style::css::{DeclBlock, Media, PageRule};
use crate::style::generated::counter_style_rules::CounterStyles;
use crate::style::values::value::{AnchorFn, RootMetrics, Sum};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

pub use arena::ArenaKey;
pub(crate) use arena::{Arena, load, store};
pub use scoped::Scoped;
pub use value_cx::{ValueCx, ValueDoc};

/// Данные одного документа. Поля пишет его разбор, читают разбор, сборка
/// кадра и фазы GPUI этого документа.
#[derive(Default)]
pub struct DocState {
    /// Ревизия и её document owner живут с отложенными ValueDoc/кадрами.
    pub(crate) budget: RefCell<Option<crate::budget::RevisionBudget>>,
    pub(crate) font_facts: font_facts::DocumentFonts,
    pub(crate) traversal: traversal::TraversalState,
    /// Первый отказ вложенного документа этой revision; не теряется при повторном кадре.
    pub(crate) embedded_failure: Cell<Option<crate::document::DocumentFailure>>,
    /// Следующий ключ синтетической коробки, независимо от авторских node_id.
    synthetic_id: RefCell<Vec<u8>>,
    /// Anchor geometry and fallback selection belonging to this document.
    pub(crate) anchor_frame: crate::layout::positioned::anchor::AnchorState,
    /// Font-face descriptors and source caches belonging to this document.
    pub(crate) font_faces: crate::text::fonts::FontFaces,
    /// Parsed MATH sources and registered native-family data of this document.
    pub(crate) font_math: crate::text::fonts::math::MathFonts,
    /// Font measurement caches scoped to this document.
    pub(crate) metrics: crate::text::metrics::Metrics,
    /// Vertical paragraph measurements and root paint geometry of this document.
    pub(crate) vertical_measures: Rc<crate::text::vertical::VerticalMeasures>,
    /// CSS font-face metric descriptors, scoped to this document.
    pub(crate) font_metric_faces: RefCell<crate::text::fonts::metric_overrides::MetricFaces>,
    /// Shaped paragraphs and fit results shared only inside this document.
    pub(crate) paragraph_measures: Rc<crate::text::paragraph::MeasureCache>,
    /// Named font-feature definitions owned by this document.
    pub(crate) font_alternates: RefCell<crate::font_facts::FeatureDefinitions>,
    /// Rasterized foreignObject layers of this document, bounded by the painter.
    pub(crate) foreign_rasters: RefCell<HashMap<u64, String>>,
    /// SVG mask/filter definitions retained by deferred groups of this document.
    pub(crate) masks: crate::resources::svg_document::masks::MaskResources,
    /// Носитель и область просмотра, которыми разобран документ.
    pub(crate) media: Cell<Media>,
    /// Режим quirks (HTML §13.2.6.4.1), ставит разбор.
    pub(crate) quirks: Cell<bool>,
    /// Кегль, строка и семейство корня — базы `rem`, `rlh`, `rcap`…
    pub(crate) root: RefCell<RootMetrics>,
    /// Left edge of the root's content-sized box under `vertical-rl`
    /// (CSS 2.2 §14.2: the canvas background is positioned from the root
    /// box). The body writes it at prepaint, the canvas layer reads it at
    /// paint of the same frame; the previous frame's value is the fallback.
    pub(crate) root_left: Cell<Option<f32>>,
    /// Правила `@page` в порядке появления (css-page-3 §cascading-and-page-context).
    pub(crate) page_rules: RefCell<Vec<PageRule>>,
    /// `@position-try <dashed-ident>` (css-anchor-position-1 §fallback-rule).
    pub(crate) try_rules: RefCell<HashMap<String, DeclBlock>>,
    /// `@property` (css-properties-values-api-1 §3).
    pub(crate) property_rules: RefCell<property_registry::PropertyRegistry>,
    /// `@counter-style` (css-counter-styles-3 §3).
    pub(crate) counter_styles: RefCell<CounterStyles>,
    /// `@color-profile`: имя → байты ICC (css-color-5 §color-profile).
    pub(crate) profiles: RefCell<HashMap<String, Vec<u8>>>,
    /// Запросы картинок документа, его база и политика доступа; декодированное — в
    /// общем процессном кэше по содержимому (`resources::images::store`).
    pub(crate) images: RefCell<crate::resources::images::store::ImageStore>,
    /// Неизменяемые факты CSS-картинок для прохода стиля, подготовленные оркестратором.
    pub(crate) background_images:
        RefCell<Rc<crate::resources::css_images::facts::BackgroundImages>>,
    /// Смешанные суммы `calc()` (`Len::Calc`).
    pub(crate) calc: Arena<Sum>,
    /// Функции `anchor()` (`Len::Anchor`).
    pub(crate) anchor: Arena<Rc<AnchorFn>>,
    /// Номера областей арен: документа и текущего кадра (`arena`).
    owner: Cell<u32>,
    frame_owner: Cell<u32>,
    /// Документ красился после последнего сброса области кадра.
    painted: Cell<bool>,
    /// Наблюдение за незавершённой работой кадра; не управляет раскладкой.
    readiness: readiness::FrameState,
    this: Weak<DocState>,
}

impl DocState {
    /// Пустое состояние документа, разбираемого носителем `media`.
    pub fn new(media: Media) -> Rc<Self> {
        let doc = Rc::new_cyclic(|this| Self {
            this: this.clone(),
            media: Cell::new(media),
            owner: Cell::new(arena::new_owner()),
            frame_owner: Cell::new(arena::new_owner()),
            ..Self::default()
        });
        doc.install_font_facts(None);
        doc.root.borrow_mut().resolve_units(doc.values());
        doc
    }

    /// Начало сборки кадра хостом: область кадра прошлого, уже покрашенного
    /// кадра больше никто не читает — сбросить.
    pub(crate) fn begin_frame(&self) {
        self.readiness.begin();
        self.images.borrow_mut().revalidate();
        self.vertical_measures.begin_frame();
        if self.painted.replace(false) {
            self.frame_owner.set(arena::new_owner());
            self.calc.clear_frame();
            self.anchor.clear_frame();
        }
    }

    /// Режим quirks документа (HTML §13.2.6.4.1).
    pub(crate) fn quirks(&self) -> bool {
        self.quirks.get()
    }

    /// Объявления правила `@position-try` по имени (`--x`), копией.
    pub(crate) fn try_rule(&self, name: &str) -> Option<crate::style::css::DeclBlock> {
        self.try_rules.borrow().get(name).cloned()
    }

    /// Неизменяемая ревизия реестра @property документа, общая для его узлов.
    pub(crate) fn property_rules(&self) -> Rc<property_registry::PropertySnapshot> {
        self.property_rules.borrow().snapshot()
    }

    /// Правила `@page` документа (копией — их читает хост на каждом кадре).
    pub fn page_rules(&self) -> Vec<PageRule> {
        self.page_rules.borrow().clone()
    }

    /// Носитель документа.
    pub fn media(&self) -> Media {
        self.media.get()
    }
}

#[cfg(test)]
mod tests;

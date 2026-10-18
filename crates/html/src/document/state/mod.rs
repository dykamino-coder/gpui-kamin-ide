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
mod scoped;
mod value_cx;

use crate::style::css::{Decls, Media, PageRule, Registered};
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
    /// Носитель и область просмотра, которыми разобран документ.
    pub(crate) media: Cell<Media>,
    /// Режим quirks (HTML §13.2.6.4.1), ставит разбор.
    pub(crate) quirks: Cell<bool>,
    /// Кегль, строка и семейство корня — базы `rem`, `rlh`, `rcap`…
    pub(crate) root: RefCell<RootMetrics>,
    /// Правила `@page` в порядке появления (css-page-3 §cascading-and-page-context).
    pub(crate) page_rules: RefCell<Vec<PageRule>>,
    /// `@position-try <dashed-ident>` (css-anchor-position-1 §fallback-rule).
    pub(crate) try_rules: RefCell<HashMap<String, Decls>>,
    /// `@property` (css-properties-values-api-1 §3).
    pub(crate) property_rules: RefCell<HashMap<String, Registered>>,
    /// `@counter-style` (css-counter-styles-3 §3).
    pub(crate) counter_styles: RefCell<CounterStyles>,
    /// `@color-profile`: имя → байты ICC (css-color-5 §color-profile).
    pub(crate) profiles: RefCell<HashMap<String, Vec<u8>>>,
    /// Смешанные суммы `calc()` (`Len::Calc`).
    pub(crate) calc: Arena<Sum>,
    /// Функции `anchor()` (`Len::Anchor`).
    pub(crate) anchor: Arena<Rc<AnchorFn>>,
    /// Номера областей арен: документа и текущего кадра (`arena`).
    owner: Cell<u32>,
    frame_owner: Cell<u32>,
    /// Документ красился после последнего сброса области кадра.
    painted: Cell<bool>,
    this: Weak<DocState>,
}

impl DocState {
    /// Пустое состояние документа, разбираемого носителем `media`.
    pub fn new(media: Media) -> Rc<Self> {
        Rc::new_cyclic(|this| Self {
            this: this.clone(),
            media: Cell::new(media),
            owner: Cell::new(arena::new_owner()),
            frame_owner: Cell::new(arena::new_owner()),
            ..Self::default()
        })
    }

    /// Начало сборки кадра хостом: область кадра прошлого, уже покрашенного
    /// кадра больше никто не читает — сбросить.
    pub(crate) fn begin_frame(&self) {
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
    pub(crate) fn try_rule(&self, name: &str) -> Option<crate::style::css::Decls> {
        self.try_rules.borrow().get(name).cloned()
    }

    /// Копия реестра `@property` — его читает сборка переменных узла.
    pub(crate) fn property_rules(&self) -> HashMap<String, Registered> {
        self.property_rules.borrow().clone()
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

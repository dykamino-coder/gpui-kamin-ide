//! Состояние, принадлежащее одному документу: его at-правила (`@page`,
//! `@position-try`, `@property`, `@counter-style`, `@color-profile`),
//! метрики корня (`rem`/`rlh`), режим quirks и носитель (`Media`).
//!
//! Владелец — `Document` (или вложенный документ рамки, `Embedded`), а не
//! процесс: так устроены Blink (`StyleEngine`/`TreeScope` документа) и Stylo
//! (`Stylist` на документ). Доступ — через стек областей потока: `enter`
//! делает документ текущим на время разбора, сборки кадра и фаз GPUI его
//! поддерева (`Scoped`), guard возвращает прежний документ при выходе и при
//! панике. Вложенный документ входит в свою область — сохранять и
//! возвращать состояние внешнего вручную не нужно.
//!
//! Арены `calc()`/`anchor()` документа и их ручки — `arena`.
//!
//! Вне любой области (тесты, `dom::parse`) действует «отсоединённый»
//! документ потока: его заводит и чистит `dom::parse_media`.

mod arena;
mod scoped;

use crate::style::css::{Decls, Media, PageRule, Registered};
use crate::style::generated::counter_style_rules::CounterStyles;
use crate::style::values::value::{AnchorFn, RootMetrics, Sum};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

pub use arena::ArenaKey;
pub(crate) use arena::{Arena, load, store};
pub use scoped::Scoped;

/// Данные одного документа. Поля пишет его разбор, читают разбор, сборка
/// кадра и фазы GPUI — всегда в области этого документа.
#[derive(Default)]
pub struct DocState {
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

thread_local! {
    /// Стек областей: вершина — документ, который сейчас разбирается,
    /// собирается или проходит фазу GPUI.
    /// Флаг — область сборки кадра (записи арен идут в область кадра).
    static STACK: RefCell<Vec<(Rc<DocState>, bool)>> = const { RefCell::new(Vec::new()) };
    /// Отсоединённый документ потока (см. заголовок модуля).
    static DETACHED: RefCell<Option<Rc<DocState>>> = const { RefCell::new(None) };
}

impl DocState {
    /// Пустое состояние документа, разбираемого носителем `media`.
    pub fn new(media: Media) -> Rc<Self> {
        Rc::new_cyclic(|this| {
            let mut state = Self::default();
            state.this = this.clone();
            state.media.set(media);
            state.owner.set(arena::new_owner(this));
            state.frame_owner.set(arena::new_owner(this));
            state
        })
    }

    /// Сделать документ текущим до конца жизни guard'а (разбор: записи
    /// арен — в область документа).
    pub fn enter(self: &Rc<Self>) -> DocScope {
        self.push(false)
    }

    /// То же для сборки кадра и фаз GPUI: записи арен — в область кадра.
    pub fn enter_frame(self: &Rc<Self>) -> DocScope {
        self.push(true)
    }

    /// Начало сборки кадра хостом: область кадра прошлого, уже покрашенного
    /// кадра больше никто не читает — сбросить.
    pub(crate) fn begin_frame(&self) {
        if self.painted.replace(false) {
            arena::forget_owner(self.frame_owner.get());
            self.frame_owner.set(arena::new_owner(&self.this));
            self.calc.clear_frame();
            self.anchor.clear_frame();
        }
    }

    fn push(self: &Rc<Self>, frame: bool) -> DocScope {
        let depth = STACK.with(|s| {
            let mut s = s.borrow_mut();
            s.push((self.clone(), frame));
            s.len()
        });
        DocScope {
            depth,
            _not_send: std::marker::PhantomData,
        }
    }

    /// Правила `@page` документа (копией — их читает хост на каждом кадре).
    pub fn page_rules(&self) -> Vec<PageRule> {
        self.page_rules.borrow().clone()
    }

    /// Носитель документа.
    pub fn media(&self) -> Media {
        self.media.get()
    }

    /// Очистить всё, что пишет разбор: перед повторным разбором в том же
    /// состоянии (отсоединённый документ потока).
    fn reset_parsed(&self, media: Media) {
        self.media.set(media);
        self.quirks.set(false);
        *self.root.borrow_mut() = RootMetrics::default();
        self.page_rules.borrow_mut().clear();
        self.try_rules.borrow_mut().clear();
        self.property_rules.borrow_mut().clear();
        self.counter_styles.borrow_mut().clear();
    }
}

impl Drop for DocState {
    fn drop(&mut self) {
        arena::forget_owner(self.owner.get());
        arena::forget_owner(self.frame_owner.get());
    }
}

/// Guard области документа: при сбросе возвращает прежний текущий документ.
pub struct DocScope {
    depth: usize,
    _not_send: std::marker::PhantomData<Rc<()>>,
}

impl Drop for DocScope {
    fn drop(&mut self) {
        STACK.with(|s| {
            let mut s = s.borrow_mut();
            debug_assert_eq!(
                s.len(),
                self.depth,
                "области документов закрыты не по порядку"
            );
            s.truncate(self.depth - 1);
        });
    }
}

/// Текущий документ: вершина стека областей, иначе отсоединённый документ
/// потока (заводится по первому требованию).
pub(crate) fn current() -> Rc<DocState> {
    current_mode().0
}

/// Текущий документ и флаг области сборки кадра.
fn current_mode() -> (Rc<DocState>, bool) {
    if let Some(top) = STACK.with(|s| s.borrow().last().cloned()) {
        return top;
    }
    let doc = DETACHED.with(|d| {
        d.borrow_mut()
            .get_or_insert_with(|| DocState::new(Media::default()))
            .clone()
    });
    (doc, false)
}

/// Отсоединённый документ потока, очищенный под новый разбор (`dom::parse`).
pub(crate) fn detached_for_parse(media: Media) -> Rc<DocState> {
    let state = DETACHED.with(|d| {
        d.borrow_mut()
            .get_or_insert_with(|| DocState::new(media))
            .clone()
    });
    state.reset_parsed(media);
    state
}

#[cfg(test)]
mod tests;

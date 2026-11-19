//! Защита рекурсивных проходов представления; исходный DOM принадлежит revision,
//! как и соответствие его элементов узлам дерева коробок (`Element::node_id`).

use super::DocumentValues;
use crate::dom::input::Admitted;
use crate::dom::outcome::{FailureScope, TraversalKind, TraversalLimit};
use markup5ever_rcdom::Handle;
use std::cell::{Cell, RefCell};

#[cfg(feature = "heap-report")]
mod heap;

/// Предел построения коробок: 2 × 1026, как nsCSSFrameConstructor Gecko.
/// Исходный DOM этим пределом не ограничен.
pub const MAX_DEPTH: usize = crate::budget::DocumentLimits::BOX_DEPTH;

#[derive(Default)]
pub struct TraversalState {
    source: RefCell<Option<Admitted>>,
    /// Адреса построенных элементов; порядок списка не является идентичностью.
    /// Ключ хранится в самом исходном узле и не зависит от style walk.
    elements: RefCell<Vec<usize>>,
    /// Адрес исходного текстового узла по его `TextId - 1`.
    texts: RefCell<Vec<usize>>,
    /// Keys of the same source texts; the traversal number is not an IFC identity.
    text_keys: RefCell<Vec<crate::dom::NodeKey>>,
    /// Адреса исходных текстов, содержимое которых прочитала сборка сверх
    /// «пустой ли узел по CSS»: первый сильный знак для `dir=auto`/`<bdi>`,
    /// текст листа `<style>`. Правка такого текста может изменить стиль.
    read: RefCell<std::collections::HashSet<usize>>,
    first_limit: Cell<Option<TraversalLimit>>,
    embedded: Cell<bool>,
    /// A computation read the viewport size (`values::viewport_reads`).
    pub viewport_read: Cell<bool>,
    /// The viewport-dependent media queries of the build being recorded
    /// (`style::css::reads::record`).
    pub media_record: RefCell<Option<crate::style::css::reads::MediaReads>>,
    /// Equal paints of this document's boxes, shared (SharedPaints).
    pub paints: crate::style::decoration_paint::SharedPaints,
}

impl TraversalState {
    pub fn retain(&self, source: Admitted) {
        self.source.replace(Some(source));
        // Обход наращивал адреса удвоением: удерживаемая копия — один точный
        // блок, выделенный после обхода, а промежуточные блоки освобождаются.
        let exact = self.elements.borrow().as_slice().to_vec();
        self.elements.replace(exact);
        let exact = self.texts.borrow().as_slice().to_vec();
        self.texts.replace(exact);
        let exact = self.text_keys.borrow().as_slice().to_vec();
        self.text_keys.replace(exact);
    }

    /// Забрать исходный DOM: следующая ревизия собирается из него же
    /// (`retain` возвращает его этой ревизии, если сборка отвергнута).
    pub fn source(&self) -> std::cell::Ref<'_, Admitted> {
        std::cell::Ref::map(self.source.borrow(), |s| s.as_ref().expect("document owns its source DOM"))
    }

    pub fn take_source(&self) -> Option<Admitted> {
        self.source.take()
    }

    /// Обход дошёл до исходного текста `handle`: его номер, следующий по
    /// порядку обхода (`None` после `u32::MAX` текстов).
    pub fn record_text(&self, handle: &Handle) -> Option<crate::dom::TextId> {
        let mut texts = self.texts.borrow_mut();
        let id = u32::try_from(texts.len() + 1).ok()?;
        texts.push(std::rc::Rc::as_ptr(handle) as usize);
        self.text_keys.borrow_mut().push(handle.key());
        crate::dom::TextId::new(id)
    }

    /// Адреса исходных текстов удерживаемого DOM по `TextId - 1`.
    pub fn texts(&self) -> std::cell::Ref<'_, Vec<usize>> {
        self.texts.borrow()
    }

    /// Persistent DOM keys by `TextId - 1`, without holding the source handles.
    pub fn text_keys(&self) -> std::cell::Ref<'_, Vec<crate::dom::NodeKey>> {
        self.text_keys.borrow()
    }

    /// Сборка прочитала содержимое исходного текста `handle`.
    pub fn read_source_text(&self, handle: &Handle) {
        self.read
            .borrow_mut()
            .insert(std::rc::Rc::as_ptr(handle) as usize);
    }

    /// Сборка прочитала содержимое текста коробок с номером `id`.
    pub fn read_text(&self, id: crate::dom::TextId) {
        let address = self.texts.borrow().get(id.get() as usize - 1).copied();
        if let Some(address) = address {
            self.read.borrow_mut().insert(address);
        }
    }

    /// Адреса исходных текстов, содержимое которых прочитала сборка.
    pub fn read_texts(&self) -> std::cell::Ref<'_, std::collections::HashSet<usize>> {
        self.read.borrow()
    }

    /// Обход построил коробку исходного элемента; его ключ выдан при создании.
    pub fn record_element(&self, handle: &Handle) {
        self.elements
            .borrow_mut()
            .push(std::rc::Rc::as_ptr(handle) as usize);
    }

    /// Адреса построенных исходных элементов удерживаемого DOM.
    pub fn elements(&self) -> std::cell::Ref<'_, Vec<usize>> {
        self.elements.borrow()
    }

    pub fn set_embedded(&self) {
        self.embedded.set(true);
    }

    /// Область, к которой относятся наблюдения этого документа.
    pub fn scope(&self) -> FailureScope {
        if self.embedded.get() {
            FailureScope::EmbeddedDocument
        } else {
            FailureScope::Document
        }
    }

    /// Запомнить первый предел ревизии и вернуть это наблюдение для журнала.
    pub fn limit(
        &self,
        traversal: TraversalKind,
        limit: usize,
        attempted: usize,
    ) -> TraversalLimit {
        let event = TraversalLimit {
            scope: self.scope(),
            traversal,
            limit,
            attempted,
        };
        if self.first_limit.get().is_none() {
            self.first_limit.set(Some(event));
        }
        event
    }
}

impl DocumentValues {
    pub fn input_context(&self) -> Option<crate::dom::input::Context> {
        self.traversal
            .source
            .borrow()
            .as_ref()
            .map(Admitted::context)
    }

    /// Проход представления упёрся в предел: статус ревизии и запись журнала.
    pub fn traversal_limit_reached(
        &self,
        traversal: TraversalKind,
        limit: usize,
        attempted: usize,
    ) {
        let event = self.traversal.limit(traversal, limit, attempted);
        self.record_diagnostic(event.into());
    }

    /// Полный исходный DOM, включая детей за пределом представления и templates.
    pub fn source_dom(&self) -> Option<Handle> {
        self.traversal
            .source
            .borrow()
            .as_ref()
            .map(|s| s.dom.document.clone())
    }

    /// Номер ключа, с которого продолжит повторный разбор этого документа.
    pub fn next_node_key(&self) -> Option<u64> {
        self.traversal
            .source
            .borrow()
            .as_ref()
            .map(|s| s.dom.next_key())
    }

    /// Первый достигнутый предел этой revision, сохраняемый между кадрами.
    pub fn traversal_limit(&self) -> Option<TraversalLimit> {
        self.traversal.first_limit.get()
    }
}

#[cfg(any(test, feature = "engine-test-support"))]
pub mod inventory_cases {
    use super::*;

    pub fn first_limit_is_retained() {
        let state = TraversalState::default();
        state.limit(TraversalKind::BoxBuild, MAX_DEPTH, MAX_DEPTH + 1);
        let first = state.first_limit.get().unwrap();
        state.limit(TraversalKind::BoxBuild, MAX_DEPTH, MAX_DEPTH + 2);
        assert_eq!(state.first_limit.get(), Some(first));
        assert_eq!((first.limit, first.attempted), (MAX_DEPTH, MAX_DEPTH + 1));
    }
}

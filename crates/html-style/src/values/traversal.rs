//! Защита рекурсивных проходов представления; исходный DOM принадлежит revision,
//! как и соответствие его элементов узлам дерева коробок (`Element::node_id`).

use super::DocumentValues;
use crate::dom::input::Admitted;
use crate::dom::outcome::{FailureScope, TraversalKind, TraversalLimit};
use markup5ever_rcdom::Handle;
use std::cell::{Cell, RefCell};

/// Предел построения коробок: 2 × 1026, как nsCSSFrameConstructor Gecko.
/// Исходный DOM этим пределом не ограничен.
pub const MAX_DEPTH: usize = crate::budget::DocumentLimits::BOX_DEPTH;

#[derive(Default)]
pub struct TraversalState {
    source: RefCell<Option<Admitted>>,
    /// Адрес исходного узла элемента по его `node_id - 1`; ноль — не записан.
    /// Адреса узлов удерживаемого `source` устойчивы, пока он удерживается.
    elements: RefCell<Vec<usize>>,
    /// Адрес исходного текстового узла по его `TextId - 1`.
    texts: RefCell<Vec<usize>>,
    /// Адреса исходных текстов, содержимое которых прочитала сборка сверх
    /// «пустой ли узел по CSS»: первый сильный знак для `dir=auto`/`<bdi>`,
    /// текст листа `<style>`. Правка такого текста может изменить стиль.
    read: RefCell<std::collections::HashSet<usize>>,
    first_limit: Cell<Option<TraversalLimit>>,
    embedded: Cell<bool>,
    /// The restyle invalidation of the batch this build rebuilds for.
    invalidation: RefCell<Option<Box<crate::dom::invalidation::Invalidation>>>,
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
    }

    /// Забрать исходный DOM: следующая ревизия собирается из него же
    /// (`retain` возвращает его этой ревизии, если сборка отвергнута).
    pub fn take_source(&self) -> Option<Admitted> {
        self.source.take()
    }

    /// Обход дошёл до исходного текста `handle`: его номер, следующий по
    /// порядку обхода (`None` после `u32::MAX` текстов).
    pub fn record_text(&self, handle: &Handle) -> Option<crate::dom::TextId> {
        let mut texts = self.texts.borrow_mut();
        let id = u32::try_from(texts.len() + 1).ok()?;
        texts.push(std::rc::Rc::as_ptr(handle) as usize);
        crate::dom::TextId::new(id)
    }

    /// Адреса исходных текстов удерживаемого DOM по `TextId - 1`.
    pub fn texts(&self) -> std::cell::Ref<'_, Vec<usize>> {
        self.texts.borrow()
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

    /// Обход вышел из исходного элемента `handle` с узлом коробок `node_id`.
    pub fn record_element(&self, handle: &Handle, node_id: u64) {
        let Some(slot) = usize::try_from(node_id)
            .ok()
            .and_then(|id| id.checked_sub(1))
        else {
            return;
        };
        let mut elements = self.elements.borrow_mut();
        if elements.len() <= slot {
            elements.resize(slot + 1, 0);
        }
        elements[slot] = std::rc::Rc::as_ptr(handle) as usize;
    }

    /// This build rebuilds the source DOM for the attribute mutations `changed`:
    /// it collects the elements they restyle (`crate::dom::invalidation`).
    pub fn invalidate(&self, changed: crate::dom::invalidation::Changed) {
        let invalidation = crate::dom::invalidation::Invalidation::new(changed);
        self.invalidation.replace(Some(Box::new(invalidation)));
    }

    /// The build knows its rules (`Invalidation::prepare`).
    pub fn invalidation_rules(&self, rules: &[crate::style::css::Rule], shadows: bool) {
        let mut invalidation = self.invalidation.take();
        if let Some(invalidation) = invalidation.as_mut() {
            invalidation.prepare(rules, shadows);
        }
        self.invalidation.replace(invalidation);
    }

    /// The walk left element `id` (`Invalidation::visit`).
    pub fn left_element(&self, tag: &str, attrs: &[(String, String)], id: u64) {
        // Outside an edit the walk pays one check per element.
        if self.invalidation.borrow().is_none() {
            return;
        }
        let mut invalidation = self.invalidation.take();
        if let Some(invalidation) = invalidation.as_mut() {
            invalidation.visit(tag, attrs, id);
        }
        self.invalidation.replace(invalidation);
    }

    /// The elements the batch of `invalidate` restyled; `None`: unknown.
    pub fn take_restyled(&self) -> Option<Vec<u64>> {
        self.invalidation.take()?.finish()
    }

    /// Адреса исходных элементов удерживаемого DOM по `node_id - 1`.
    pub fn elements(&self) -> std::cell::Ref<'_, Vec<usize>> {
        self.elements.borrow()
    }

    /// Следующая ревизия с тем же деревом коробок: её исходный DOM и адреса его
    /// элементов и текстов по тем же `node_id` и `TextId` и прочитанных сборкой
    /// текстов.
    pub fn readopt(
        &self,
        source: Admitted,
        elements: Vec<usize>,
        texts: Vec<usize>,
        read: std::collections::HashSet<usize>,
    ) {
        self.source.replace(Some(source));
        self.elements.replace(elements);
        self.texts.replace(texts);
        self.read.replace(read);
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

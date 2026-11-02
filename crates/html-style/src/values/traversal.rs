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
    first_limit: Cell<Option<TraversalLimit>>,
    embedded: Cell<bool>,
}

impl TraversalState {
    pub fn retain(&self, source: Admitted) {
        self.source.replace(Some(source));
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

    /// Адреса исходных элементов удерживаемого DOM по `node_id - 1`.
    pub fn elements(&self) -> std::cell::Ref<'_, Vec<usize>> {
        self.elements.borrow()
    }

    /// Следующая ревизия с тем же деревом коробок: её исходный DOM и адреса его
    /// элементов по тем же `node_id`.
    pub fn readopt(&self, source: Admitted, elements: Vec<usize>) {
        self.source.replace(Some(source));
        self.elements.replace(elements);
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

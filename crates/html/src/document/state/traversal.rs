//! Защита рекурсивных проходов представления; исходный DOM принадлежит revision.

use super::DocState;
use crate::document::{FailureScope, TraversalKind, TraversalLimit};
use crate::dom::input::Admitted;
use markup5ever_rcdom::Handle;
use std::cell::{Cell, RefCell};

/// Предел построения коробок: 2 × 1026, как nsCSSFrameConstructor Gecko.
/// Исходный DOM этим пределом не ограничен.
pub(crate) const MAX_DEPTH: usize = crate::budget::DocumentLimits::BOX_DEPTH;

#[derive(Default)]
pub(crate) struct TraversalState {
    source: RefCell<Option<Admitted>>,
    first_limit: Cell<Option<TraversalLimit>>,
    embedded: Cell<bool>,
}

impl TraversalState {
    pub(crate) fn retain(&self, source: Admitted) {
        *self.source.borrow_mut() = Some(source);
    }

    pub(crate) fn set_embedded(&self) {
        self.embedded.set(true);
    }

    /// Область, к которой относятся наблюдения этого документа.
    pub(crate) fn scope(&self) -> FailureScope {
        if self.embedded.get() {
            FailureScope::EmbeddedDocument
        } else {
            FailureScope::Document
        }
    }

    /// Запомнить первый предел ревизии и вернуть это наблюдение для журнала.
    pub(crate) fn limit(
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

impl DocState {
    /// Проход представления упёрся в предел: статус ревизии и запись журнала.
    pub(crate) fn traversal_limit_reached(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_limit_is_retained() {
        let state = TraversalState::default();
        state.limit(TraversalKind::BoxBuild, MAX_DEPTH, MAX_DEPTH + 1);
        let first = state.first_limit.get().unwrap();
        state.limit(TraversalKind::BoxBuild, MAX_DEPTH, MAX_DEPTH + 2);
        assert_eq!(state.first_limit.get(), Some(first));
        assert_eq!((first.limit, first.attempted), (MAX_DEPTH, MAX_DEPTH + 1));
    }
}

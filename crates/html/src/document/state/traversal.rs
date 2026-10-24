//! Защита рекурсивных проходов представления; исходный DOM принадлежит revision.

use super::DocState;
use crate::document::{FailureScope, TraversalKind, TraversalLimit};
use crate::dom::input::Admitted;
use markup5ever_rcdom::Handle;
use std::cell::{Cell, RefCell};

/// Подъём требует измерения кадров стека и итеративных проходов (deepstack).
pub(crate) const MAX_DEPTH: usize = 64;

#[derive(Default)]
pub(crate) struct TraversalState {
    source: RefCell<Option<Admitted>>,
    depth: Cell<usize>,
    first_limit: Cell<Option<TraversalLimit>>,
    embedded: Cell<bool>,
}

pub(crate) struct TraversalGuard<'a>(&'a TraversalState);

impl Drop for TraversalGuard<'_> {
    fn drop(&mut self) {
        self.0.depth.set(self.0.depth.get() - 1);
    }
}

impl TraversalState {
    pub(crate) fn retain(&self, source: Admitted) {
        *self.source.borrow_mut() = Some(source);
    }

    pub(crate) fn set_embedded(&self) {
        self.embedded.set(true);
    }

    pub(crate) fn enter(&self, traversal: TraversalKind) -> Option<TraversalGuard<'_>> {
        let depth = self.depth.get() + 1;
        if depth > MAX_DEPTH {
            self.limit(traversal, MAX_DEPTH, depth);
            return None;
        }
        self.depth.set(depth);
        Some(TraversalGuard(self))
    }

    pub(crate) fn limit(&self, traversal: TraversalKind, limit: usize, attempted: usize) {
        if self.first_limit.get().is_none() {
            self.first_limit.set(Some(TraversalLimit {
                scope: if self.embedded.get() {
                    FailureScope::EmbeddedDocument
                } else {
                    FailureScope::Document
                },
                traversal,
                limit,
                attempted,
            }));
        }
    }
}

impl DocState {
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
    fn recursive_guard_unwinds_and_reports_on_a_small_stack() {
        std::thread::Builder::new()
            .stack_size(128 * 1024)
            .spawn(|| {
                let state = TraversalState::default();
                fn walk(state: &TraversalState) -> usize {
                    let Some(_guard) = state.enter(TraversalKind::BoxBuild) else {
                        return 0;
                    };
                    1 + walk(state)
                }
                assert_eq!(walk(&state), MAX_DEPTH);
                let limit = state.first_limit.get().unwrap();
                assert_eq!((limit.limit, limit.attempted), (64, 65));
                assert!(state.enter(TraversalKind::BoxBuild).is_some());
                assert_eq!(state.first_limit.get(), Some(limit));
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

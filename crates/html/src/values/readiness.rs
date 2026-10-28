//! Наблюдение за работой кадра: отложенный layout и вложенные документы.
//! Флаг читается хостом только после завершения GPU-кадра. Шрифты загружаются
//! синхронно при разборе; ImageStore учитывает загрузку и декодирование картинок.

use super::DocumentValues;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Default)]
pub(super) struct FrameState {
    deferred: Cell<bool>,
    children: RefCell<Vec<Rc<DocumentValues>>>,
}

impl FrameState {
    pub(super) fn begin(&self) {
        self.deferred.set(false);
        self.children.borrow_mut().clear();
    }
}

impl DocumentValues {
    /// Существующий проход потребовал следующую раскладку. Только наблюдение:
    /// вызов не планирует кадры и не меняет результат layout/paint.
    pub(crate) fn defer_frame(&self) {
        self.readiness.deferred.set(true);
    }

    /// Вложенный документ должен закончить работу вместе с внешним.
    pub(crate) fn observe_child(&self, child: Rc<DocumentValues>) {
        self.readiness.children.borrow_mut().push(child);
    }

    /// Вложенные документы текущего кадра (копия списка, без удержания borrow).
    pub(super) fn observed_children(&self) -> Vec<Rc<DocumentValues>> {
        self.readiness.children.borrow().clone()
    }

    /// Вложенные failures возникают и при отложенной сборке дочернего кадра.
    pub(crate) fn embedded_failure(&self) -> Option<crate::dom::outcome::DocumentFailure> {
        self.embedded_failure.get().or_else(|| {
            self.readiness
                .children
                .borrow()
                .iter()
                .find_map(|child| child.embedded_failure())
        })
    }

    pub(crate) fn child_traversal_limit(&self) -> Option<crate::dom::outcome::TraversalLimit> {
        self.readiness.children.borrow().iter().find_map(|child| {
            child
                .traversal_limit()
                .or_else(|| child.child_traversal_limit())
        })
    }

    pub(crate) fn frame_ready(&self) -> bool {
        self.painted.get()
            && !self.readiness.deferred.get()
            && !self.images.borrow().has_pending()
            && self
                .readiness
                .children
                .borrow()
                .iter()
                .all(|child| child.frame_ready())
    }
}

#[cfg(test)]
mod tests {
    use crate::values::DocumentValues;

    #[test]
    fn nested_terminal_failure_is_visible_to_the_root_document() {
        let root = DocumentValues::detached(Default::default());
        let child = DocumentValues::detached(Default::default());
        let grandchild = DocumentValues::detached(Default::default());
        let mut failure = crate::dom::outcome::DocumentFailure::new(
            crate::dom::outcome::FailureReason::XmlSyntax { row: 1, column: 2 },
        );
        failure.scope = crate::dom::outcome::FailureScope::EmbeddedDocument;
        grandchild.embedded_failure.set(Some(failure));
        child.observe_child(grandchild.clone());
        root.observe_child(child.clone());
        assert_eq!(root.embedded_failure(), Some(failure));
    }

    #[test]
    fn nested_traversal_limit_reaches_the_root_without_rejecting_it() {
        let root = DocumentValues::detached(Default::default());
        let child = DocumentValues::detached(Default::default());
        let grandchild = DocumentValues::detached(Default::default());
        grandchild.traversal.set_embedded();
        grandchild
            .traversal
            .limit(crate::dom::outcome::TraversalKind::BoxBuild, 64, 65);
        let expected = grandchild.traversal_limit();
        child.observe_child(grandchild.clone());
        root.observe_child(child.clone());
        assert_eq!(root.child_traversal_limit(), expected);
        assert!(root.embedded_failure().is_none());
    }

    #[test]
    fn paint_and_all_nested_work_must_finish() {
        let doc = DocumentValues::detached(Default::default());
        let child = DocumentValues::detached(Default::default());
        assert!(!doc.frame_ready());
        doc.painted.set(true);
        assert!(doc.frame_ready());
        doc.observe_child(child.clone());
        assert!(!doc.frame_ready());
        child.painted.set(true);
        assert!(doc.frame_ready());
        child.defer_frame();
        assert!(!doc.frame_ready());
        doc.begin_frame_inputs();
        doc.reset_painted_frame();
        assert!(!doc.frame_ready());
        doc.painted.set(true);
        assert!(doc.frame_ready());
    }
}

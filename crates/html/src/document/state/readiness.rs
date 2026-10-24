//! Наблюдение за работой кадра: отложенный layout и вложенные документы.
//! Флаг читается хостом только после завершения GPU-кадра. Шрифты загружаются
//! синхронно при разборе; ImageStore учитывает загрузку и декодирование картинок.

use super::DocState;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Default)]
pub(super) struct FrameState {
    deferred: Cell<bool>,
    children: RefCell<Vec<Rc<DocState>>>,
}

impl FrameState {
    pub(super) fn begin(&self) {
        self.deferred.set(false);
        self.children.borrow_mut().clear();
    }
}

impl DocState {
    /// Существующий проход потребовал следующую раскладку. Только наблюдение:
    /// вызов не планирует кадры и не меняет результат layout/paint.
    pub(crate) fn defer_frame(&self) {
        self.readiness.deferred.set(true);
    }

    /// Вложенный документ должен закончить работу вместе с внешним.
    pub(crate) fn observe_child(&self, child: Rc<DocState>) {
        self.readiness.children.borrow_mut().push(child);
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
    use super::*;

    #[test]
    fn paint_and_all_nested_work_must_finish() {
        let doc = DocState::new(Default::default());
        let child = DocState::new(Default::default());
        assert!(!doc.frame_ready());
        doc.painted.set(true);
        assert!(doc.frame_ready());
        doc.observe_child(child.clone());
        assert!(!doc.frame_ready());
        child.painted.set(true);
        assert!(doc.frame_ready());
        child.defer_frame();
        assert!(!doc.frame_ready());
        doc.begin_frame();
        assert!(!doc.frame_ready());
        doc.painted.set(true);
        assert!(doc.frame_ready());
    }
}

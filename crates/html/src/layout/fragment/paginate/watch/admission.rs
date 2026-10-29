//! Собственный допуск узлов в записи главной копии, воспроизводимой по кускам
//! (`ColumnStack::replay_routes`), и допуск узла, который рисуется сейчас: художники
//! фрагмента внутри допущенного узла режут себя по его срезам, а не по кускам ребёнка.

use super::Watched;
use gpui::PaintSegmentSlice;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

type Slices = Rc<[PaintSegmentSlice]>;

#[derive(Default)]
pub(crate) struct Admission {
    nodes: RefCell<HashMap<u64, Slices>>,
    current: Cell<Option<Slices>>,
}

impl Watched {
    /// Допуск узлов на время записи или prepaint главной копии (пустой — снят).
    pub(crate) fn set_admission(&self, nodes: HashMap<u64, Slices>) {
        self.admission.nodes.replace(nodes);
    }

    pub(super) fn admitted(&self, id: u64) -> Option<Slices> {
        self.admission.nodes.borrow().get(&id).cloned()
    }

    /// Краска допущенного узла: его срезы действуют для потомков до возврата.
    pub(super) fn with_admitted<R>(&self, slices: Slices, f: impl FnOnce() -> R) -> R {
        let previous = self.admission.current.replace(Some(slices));
        let result = f();
        self.admission.current.set(previous);
        result
    }

    /// Срезы ближайшего допущенного узла, который рисуется сейчас.
    pub(crate) fn current_admitted(&self) -> Option<Slices> {
        let current = self.admission.current.take();
        self.admission.current.set(current.clone());
        current
    }
}

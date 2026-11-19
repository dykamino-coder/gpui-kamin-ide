//! Устойчивая невладеющая идентичность исходного DOM: номер, выданный документом.
//!
//! Документ выдаёт номера подряд из своего счётчика и никогда не выдаёт номер
//! повторно, поэтому ключ умершего узла не может указать на другой узел (ABA
//! нет), а один и тот же вход даёт одни и те же ключи независимо от других
//! документов процесса. Узел хранит только свой номер: ни реестра в узле, ни
//! таблицы слотов, ни работы при Drop (Blink хранит id в узле тоже, но ведёт
//! процессный счётчик и карту id → узел, `dom_node_ids.cc`; карта здесь не
//! нужна, пока ключ никто не разрешает обратно).

use crate::{
    admission::{self, Admission},
    Handle, Node, NodeData, RcDom,
};
use std::cell::Cell;

/// POD-контракт хоста: непрозрачный номер. Ни плотности, ни порядка документа
/// он не несёт; имеет смысл только внутри документа, который его выдал.
/// Ноль — «без ключа»: узел, созданный вне документа, и синтетика хоста.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct NodeKey(u64);

impl NodeKey {
    /// Ключ узла, созданного вне документа; документ выдаёт ключ при `adopt`.
    pub const NONE: Self = Self(0);

    /// Представление для дерева коробок; 0 оставлен синтетике.
    pub const fn packed(self) -> u64 {
        self.0
    }

    pub const fn from_packed(value: u64) -> Self {
        Self(value)
    }
}

/// Счётчик ключей одного документа: следующий номер к выдаче.
pub(crate) struct Keys(Cell<u64>);

impl Keys {
    /// Повторный разбор продолжает номера прежнего DOM, чтобы его ещё живые
    /// узлы не совпали по ключу с новыми.
    pub(crate) fn starting_at(first: u64) -> Self {
        Self(Cell::new(first.max(1)))
    }

    pub(crate) fn issue<A: Admission>(&self, policy: &A) -> Result<NodeKey, A::Error> {
        let key = self.0.get();
        // Переполнение u64 не наступает на практике, но номер не повторяется.
        let next = key.checked_add(1).ok_or_else(|| policy.overflow())?;
        self.0.set(next);
        Ok(NodeKey(key))
    }
}

impl Node {
    pub fn key(&self) -> NodeKey {
        self.key.get()
    }
}

impl RcDom {
    /// Номер, который получит следующий созданный узел этого документа.
    pub fn next_key(&self) -> u64 {
        self.keys.0.get()
    }

    /// Создать узел с ключом этого документа; для bounded-кода — TreeSink.
    pub fn new_node(&self, data: NodeData) -> Handle {
        admission::infallible(self.try_new_node(data, None, &admission::Unbounded))
    }

    pub(crate) fn try_new_node<A: Admission>(
        &self,
        data: NodeData,
        attrs_lease: admission::Lease,
        policy: &A,
    ) -> Result<Handle, A::Error> {
        let key = self.keys.issue(policy)?;
        Node::try_new(data, attrs_lease, policy, key)
    }

    /// Принять поддерево, созданное вне документа или в другом документе:
    /// каждый его узел получает новый ключ этого документа (запись мутации
    /// adopt). Ключи прежнего документа здесь ничего не значат.
    pub fn adopt(&self, root: &Handle) {
        let mut pending = vec![root.clone()];
        while let Some(node) = pending.pop() {
            node.key.set(admission::infallible(
                self.keys.issue(&admission::Unbounded),
            ));
            pending.extend(node.children.borrow().iter().cloned());
            if let NodeData::Element {
                template_contents, ..
            } = &node.data
            {
                pending.extend(template_contents.borrow().iter().cloned());
            }
        }
    }
}

#[cfg(test)]
mod tests;

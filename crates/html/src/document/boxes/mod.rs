//! Дерево коробок документа: стабильные слоты и изменяемые связи между ними.
//!
//! Входное Vec<Node> пока служит адаптером подготовки. Арена не хранит его снимков:
//! публикация меняет существующие записи, а удаление увеличивает поколение слота.

mod adapter;
mod arena;
pub(crate) mod atom_styles;
#[cfg(test)]
mod box_of_tests;
mod by_node;
mod epochs;
pub(crate) use epochs::Epochs;
#[cfg(test)]
mod epochs_tests;
mod finish;
mod flows;
pub(crate) use finish::MixedText;
#[cfg(feature = "heap-report")]
mod heap;
pub(crate) mod inline;
#[cfg(test)]
mod key_tests;
mod level;
mod links;
mod publish;
mod reuse;
mod source;
#[cfg(test)]
mod source_tests;
mod sources;
#[cfg(test)]
mod tests;
pub(crate) mod used;

use crate::dom::{BoxId, Node};
use arena::Arena;
use source::Source;
use std::rc::Rc;

pub(crate) struct BoxRecord {
    pub(crate) parent: BoxId,
    pub(crate) first: BoxId,
    pub(crate) next: BoxId,
    source: u64,
    origin: Source,
    pub(crate) inline: Option<Rc<inline::InlineContent>>,
    prepared: Option<Box<finish::Prepared>>,
    /// Номера публикаций, на которых изменились входы раскладки.
    pub(crate) epochs: Epochs,
    /// Вклад в intrinsic-размеры предка не зависит от содержимого.
    external: bool,
    /// The box sizes its children from all their intrinsic contributions
    /// (table, flex, grid): a child's width follows its siblings' content.
    distributes: bool,
    /// Удерживаемый результат раскладки коробки; им владеет раскладка.
    layout_slot: Option<Box<crate::layout::retained::LayoutSlot>>,
    /// Box style of the atomic inline of this box from an earlier frame (`atom_styles`).
    atom_style: Option<Rc<atom_styles::AtomStyle>>,
}

#[derive(Default)]
pub(crate) struct BoxTree {
    arena: Arena,
    flows: flows::Flows,
    /// The first box of each keyed source element (`Element::node_id`):
    /// layout finds its box in O(1); fragment copies share it.
    by_node: by_node::ByNode,
    first: BoxId,
    epoch: u64,
    /// Номер публикации для эпох раскладки (`Epochs`).
    clock: u32,
    finished: Option<(u64, crate::text::inline::normalization::Inputs)>,
    /// Потоки, обойдённые последней публикацией, до подготовки кадром.
    visited: finish::Visited,
    /// Used styles derived from computed ones, shared by the boxes of one style.
    pub(crate) used: used::UsedStyles,
    /// The parent box style of the latest atoms (`atom_styles`).
    atom_parent: Option<Rc<crate::style::computed::BoxStyle>>,
}

impl BoxTree {
    pub(crate) fn is_empty(&self) -> bool {
        !self.first.is_some()
    }

    /// The one mutable entry to a document's box tree. Used styles in it are
    /// shared values (`used`): no caller reaches a `Computed` through it.
    pub(crate) fn edit(tree: &std::cell::RefCell<Self>) -> std::cell::RefMut<'_, Self> {
        tree.borrow_mut()
    }

    /// The box of the source element with key `node_id` (its first fragment).
    pub(crate) fn box_of(&self, node_id: u64) -> Option<BoxId> {
        self.by_node.get(node_id)
    }

    pub(crate) fn get(&self, id: BoxId) -> Option<&BoxRecord> {
        self.arena.get(id)
    }

    /// Живые коробки без обхода адаптерного дерева Element/Children.
    /// Порядок слотов не задаёт порядок документа: его задают first/next.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (BoxId, &BoxRecord)> {
        self.arena.iter()
    }

    /// Опубликовать принятый вход той же формы.
    /// Источник задаётся NodeKey DOM, а синтетическая роль — отдельно от него.
    #[cfg(test)]
    fn publish(&mut self, nodes: &[Node]) {
        self.publish_sources(nodes, &[]);
    }

    /// `previous` — вход прежней публикации, ещё живой: по нему ставятся эпохи
    /// раскладки; без него все коробки считаются изменёнными.
    pub(crate) fn publish_document(
        &mut self,
        nodes: &[Node],
        previous: Option<&[Node]>,
        values: &crate::values::DocumentValues,
    ) {
        let text = values.traversal.text_keys();
        let element = |id| Some(crate::dom::NodeKey::from_packed(id));
        let keys = publish::Keys {
            text: &text,
            element: &element,
        };
        self.publish_keyed(nodes, previous, &keys);
    }

    #[cfg(test)]
    fn publish_sources(&mut self, nodes: &[Node], text_keys: &[crate::dom::NodeKey]) {
        let element = |id| Some(crate::dom::NodeKey::from_packed(id));
        let keys = publish::Keys {
            text: text_keys,
            element: &element,
        };
        self.publish_keyed(nodes, None, &keys);
    }

    /// Полная смена формы до появления постоянных ключей DOM: старые ключи
    /// коробок инвалидируются, а слоты получают новые поколения.
    pub(crate) fn replace(&mut self, nodes: &[Node], values: &crate::values::DocumentValues) {
        let ids: Vec<_> = self.iter().map(|(id, _)| id).collect();
        for id in ids {
            self.arena.remove(id);
        }
        self.first = BoxId::NONE;
        self.by_node.clear();
        self.flows.clear();
        self.publish_document(nodes, None, values);
    }
}

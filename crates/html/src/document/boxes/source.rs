//! Ключ источника коробки: POD DOM-узла отдельно от синтетической роли.
//!
//! До подключения M1 тип повторяет его зафиксированный ABI. Числовой транспорт
//! Element::node_id здесь только распаковывается, его порядок не используется.

use crate::dom::Element;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct NodeKey {
    pub(super) slot: u32,
    pub(super) generation: u32,
}

impl NodeKey {
    fn from_packed(value: u64) -> Self {
        Self {
            slot: value as u32,
            generation: (value >> 32) as u32,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum Source {
    Node(NodeKey),
    Synthetic(String),
}

impl Source {
    pub(super) fn of(element: &Element) -> Self {
        if element.node_id != 0 {
            Self::Node(NodeKey::from_packed(element.node_id))
        } else {
            Self::Synthetic(element.marks.state_id.get().cloned().unwrap_or_default())
        }
    }
}

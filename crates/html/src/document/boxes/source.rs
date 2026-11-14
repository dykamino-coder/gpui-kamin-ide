//! Ключ источника коробки: POD DOM-узла отдельно от синтетической роли.
//!
//! Числовой транспорт Element::node_id только распаковывается проектным типом
//! M1; порядок и плотность ключей не используются.

use crate::dom::Element;
pub(super) use crate::dom::NodeKey;
use std::string::String;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum Source {
    Root,
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

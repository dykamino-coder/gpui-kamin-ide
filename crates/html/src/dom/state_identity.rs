//! Ключи состояния коробок GPUI: роль обёртки и отдельная идентичность synthetic.

use super::{Element, Node};
use crate::document::state::ValueCx;

impl Element {
    /// Роли не пересекаются; синтетические ключи отделены от авторских номеров.
    pub(crate) fn state_id(&self, role: &'static str, vcx: ValueCx<'_>) -> gpui::ElementId {
        if self.node_id != 0 {
            return gpui::ElementId::NamedInteger(role.into(), self.node_id);
        }
        let id = self.synthetic_state_id(vcx);
        gpui::ElementId::Name(format!("{role}:synthetic:{id}").into())
    }

    fn synthetic_state_id(&self, vcx: ValueCx<'_>) -> &str {
        self.marks
            .state_id
            .get_or_init(|| vcx.doc.next_synthetic_id())
    }
}

/// До кадровых клонов закрепить идентичность всех сгенерированных DOM-коробок.
/// Поздние layout-коробки получают её лениво при первом запросе состояния.
pub(super) fn finalize(nodes: &[Node], vcx: ValueCx<'_>) {
    for node in nodes {
        if let Node::Element(e) = node {
            if e.node_id == 0 {
                e.synthetic_state_id(vcx);
            }
            finalize(&e.children, vcx);
        }
    }
}

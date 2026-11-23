//! Ключи состояния коробок GPUI: роль обёртки и отдельная идентичность synthetic.

use super::{Element, Node};
use crate::values::ValueCx;

impl Element {
    /// Роли не пересекаются; синтетические ключи отделены от авторских номеров.
    pub fn state_id(&self, role: &'static str, vcx: ValueCx<'_>) -> gpui::ElementId {
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

/// Assign before frame clones; the pass retains only the identity namespace.
pub(super) fn finalize(
    admission: &crate::budget::admission::Admission,
    nodes: Vec<Node>,
    vcx: ValueCx<'_>,
) -> Result<Vec<Node>, crate::budget::admission::Error> {
    super::normalization::apply(
        &StateIdentity,
        nodes,
        IdentityNamespace(vcx.doc.synthetic_ids()),
        admission,
    )
}

#[derive(Clone)]
pub(super) struct IdentityNamespace(pub crate::values::SyntheticIds);

impl PartialEq for IdentityNamespace {
    fn eq(&self, other: &Self) -> bool {
        self.0.same_allocator(&other.0)
    }
}

pub(super) struct StateIdentity;

impl super::normalization::Transform for StateIdentity {
    type Context = IdentityNamespace;

    fn enter(
        &self,
        element: &mut Element,
        namespace: &IdentityNamespace,
        _: &crate::budget::admission::Admission,
    ) -> Result<IdentityNamespace, crate::budget::admission::Error> {
        if element.node_id == 0 {
            element.marks.state_id.get_or_init(|| namespace.0.next());
        }
        Ok(namespace.clone())
    }
}

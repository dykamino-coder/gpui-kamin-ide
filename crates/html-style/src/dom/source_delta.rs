//! Net source mutations shared by the DOM writer and cascade consumer.
//!
//! The journal is captured before DOM writes, so removed links and qualified
//! attribute names survive rollback and do not require reading a newer tree.

use super::NodeKey;
use html5ever::QualName;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SourceDelta {
    pub changes: Vec<SourceChange>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceChange {
    pub node: NodeKey,
    pub kind: SourceChangeKind,
    pub before_parent: Option<NodeKey>,
    pub after_parent: Option<NodeKey>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceChangeKind {
    Attributes(Vec<QualName>),
    CharacterData,
    Children,
    Reparent,
}

impl SourceDelta {
    pub fn changes_children(&self) -> bool {
        self.changes.iter().any(|change| {
            matches!(
                change.kind,
                SourceChangeKind::Children | SourceChangeKind::Reparent
            )
        })
    }
}

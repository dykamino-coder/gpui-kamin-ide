//! Итоговые изменения DOM до записи: ключи не зависят от видимости или обхода.

use super::{Batch, parent};
use crate::document::delta::{SourceChange, SourceChangeKind, SourceDelta};
use html5ever::{Attribute, QualName};

impl Batch {
    pub(super) fn source_delta(&self) -> SourceDelta {
        let parents = self.children.changed_parents();
        let mut delta = SourceDelta::default();
        let mut push = |node: &markup5ever_rcdom::Handle, kind| {
            let (before_parent, after_parent) =
                parents.get(&node.key()).copied().unwrap_or_else(|| {
                    let parent = parent(node).map(|parent| parent.key());
                    (parent, parent)
                });
            delta.changes.push(SourceChange {
                node: node.key(),
                kind,
                before_parent,
                after_parent,
            });
        };
        for change in &self.elements {
            push(
                &change.element,
                SourceChangeKind::Attributes(names(&change.old, &change.new)),
            );
        }
        for change in &self.texts {
            push(&change.node, SourceChangeKind::CharacterData);
        }
        self.children.append_delta(&mut delta, &parents);
        delta
    }

    /// Порядок attribute-событий сохраняет порядок свёрнутых scalar-записей.
    pub(in crate::document::revision) fn attribute_deltas(
        &self,
    ) -> impl Iterator<Item = (&super::AttrChange, &[QualName])> {
        let names = self
            .delta
            .changes
            .iter()
            .filter_map(|change| match &change.kind {
                SourceChangeKind::Attributes(names) => Some(names.as_slice()),
                _ => None,
            });
        self.elements.iter().zip(names)
    }
}

fn names(old: &[Attribute], new: &[Attribute]) -> Vec<QualName> {
    let mut names = Vec::new();
    for attr in old.iter().chain(new) {
        if value(old, &attr.name) != value(new, &attr.name) && !names.contains(&attr.name) {
            names.push(attr.name.clone());
        }
    }
    names
}

pub(in crate::document::revision) fn value<'a>(
    list: &'a [Attribute],
    name: &QualName,
) -> Option<&'a str> {
    list.iter().find(|a| a.name == *name).map(|a| &*a.value)
}

#[cfg(test)]
mod tests;

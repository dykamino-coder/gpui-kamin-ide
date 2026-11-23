//! The source DOM mutation journal used by document revisions.
//!
//! Each record names a source node. A document-owned handle also identifies
//! a detached node. Child records address both ends of the changed link.
//! `Document::update` converts same-shape markup edits into this journal.
//!
//! A batch keeps old values, child positions and parent links for rollback.

mod attributes;
mod children;
mod delta;
mod source;
pub(super) use attributes::attribute_records;
pub(super) use delta::value as attribute_value;
pub use source::SourceNode;
pub(super) use source::open;
use source::parent;

use super::apply::ApplyError;
use html5ever::{Attribute, QualName};
use markup5ever_rcdom::{Handle, NodeData};
use std::rc::Rc;

/// Запись мутации исходного DOM ревизии.
#[derive(Clone)]
pub enum Mutation {
    /// Insert or move this node before a direct child, or append when before is None.
    InsertBefore {
        parent: SourceNode,
        child: SourceNode,
        before: Option<Handle>,
    },
    /// DOM removeChild: detach this direct child of the given source parent.
    RemoveChild { parent: Handle, child: Handle },
    /// Новое содержимое текстового узла `node`.
    CharacterData { node: Handle, data: String },
    /// Атрибут `name` элемента `element`: новое значение или `None` — снять.
    Attribute {
        element: Handle,
        name: QualName,
        value: Option<String>,
    },
}

/// Правка одного текста пакета: прежнее и новое содержимое.
pub(super) struct TextChange {
    pub(super) node: Handle,
    pub(super) old: String,
    pub(super) new: String,
}

/// Правка атрибутов одного элемента пакета: прежний и итоговый списки.
pub(super) struct AttrChange {
    pub(super) element: Handle,
    pub(super) old: Vec<Attribute>,
    pub(super) new: Vec<Attribute>,
}

/// Записи пакета, сведённые к узлам; правки без итогового отличия выброшены.
#[derive(Default)]
pub(super) struct Batch {
    children: children::Changes,
    pub(super) texts: Vec<TextChange>,
    pub(super) elements: Vec<AttrChange>,
    pub(super) delta: crate::document::delta::SourceDelta,
}

type Address = *const markup5ever_rcdom::Node;

/// One target needs no table allocation. Additional targets index the text
/// or attribute vector for the node's fixed storage kind.
#[derive(Default)]
struct ScalarSlots {
    first: Option<Address>,
    others: std::collections::HashMap<Address, usize>,
}

impl ScalarSlots {
    fn slot(&mut self, node: &Handle, next: usize) -> usize {
        let address = Rc::as_ptr(node);
        if *self.first.get_or_insert(address) == address {
            0
        } else {
            *self.others.entry(address).or_insert(next)
        }
    }
}

impl Batch {
    /// Shape comparison already paired every target with the original source DOM.
    pub(super) fn new(records: &[Mutation]) -> Option<Self> {
        Self::collect(records, None, || {
            unreachable!("shape comparison emits scalar records")
        })
        .ok()
    }

    pub(super) fn in_document(
        records: &[Mutation],
        root: &Handle,
        context: impl FnOnce() -> crate::dom::input::Context,
    ) -> Result<Self, ApplyError> {
        Self::collect(records, Some(root), context)
    }

    fn collect(
        records: &[Mutation],
        root: Option<&Handle>,
        context: impl FnOnce() -> crate::dom::input::Context,
    ) -> Result<Self, ApplyError> {
        let mut batch = Self::default();
        let mut children = children::Requests::default();
        let mut certified = std::collections::HashSet::new();
        let mut slots = ScalarSlots::default();
        for record in records {
            match record {
                Mutation::InsertBefore {
                    parent,
                    child,
                    before,
                } => {
                    if !root.is_some_and(|r| child.owned_by(r) && parent.owned_by(r)) {
                        return Err(ApplyError::Foreign);
                    }
                    certified.insert(Rc::as_ptr(child.handle()));
                    certified.insert(Rc::as_ptr(parent.handle()));
                    if let Some(from) = source::parent(child.handle()) {
                        certified.insert(Rc::as_ptr(&from));
                    }
                    children.insert(parent.handle(), child.handle(), before.as_ref())?;
                }
                Mutation::RemoveChild { parent, child } => {
                    children.remove(parent, child)?;
                }
                Mutation::CharacterData { node, data } => {
                    let NodeData::Text { contents } = &node.data else {
                        return Err(ApplyError::Foreign);
                    };
                    let at = slots.slot(node, batch.texts.len());
                    if at == batch.texts.len() {
                        let old = contents.borrow().to_string();
                        batch.texts.push(TextChange {
                            node: node.clone(),
                            new: String::new(),
                            old,
                        });
                    }
                    data.clone_into(&mut batch.texts[at].new);
                }
                Mutation::Attribute {
                    element,
                    name,
                    value,
                } => {
                    let NodeData::Element { attrs, .. } = &element.data else {
                        return Err(ApplyError::Foreign);
                    };
                    let at = slots.slot(element, batch.elements.len());
                    if at == batch.elements.len() {
                        let old = attrs.borrow().clone();
                        batch.elements.push(AttrChange {
                            element: element.clone(),
                            new: old.clone(),
                            old,
                        });
                    }
                    attributes::set(&mut batch.elements[at].new, name, value.as_deref());
                }
            }
        }
        if let Some(root) = root {
            let mut targets = batch
                .texts
                .iter()
                .map(|t| &t.node)
                .chain(batch.elements.iter().map(|e| &e.element))
                .chain(children.parents());
            if !targets.all(|node| source::belongs(node, root, &certified)) {
                return Err(ApplyError::Foreign);
            }
        }
        batch.children = children.finish(context)?;
        batch.texts.retain(|t| t.old != t.new);
        batch.elements.retain(|e| e.old != e.new);
        batch.delta = batch.source_delta();
        Ok(batch)
    }

    pub(super) fn changes_children(&self) -> bool {
        self.delta.changes_children()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.delta.changes.is_empty()
    }

    /// Записать итоговые значения в исходный DOM.
    pub(super) fn write(&mut self) {
        self.store(|t| &t.new, |e| &e.new);
        self.children.exchange(true);
    }

    /// Вернуть прежние значения: пакет отвергнут.
    pub(super) fn undo(&mut self) {
        self.children.exchange(false);
        self.store(|t| &t.old, |e| &e.old);
    }

    fn store(
        &self,
        text: impl Fn(&TextChange) -> &String,
        attrs: impl Fn(&AttrChange) -> &Vec<Attribute>,
    ) {
        for t in &self.texts {
            if let NodeData::Text { contents } = &t.node.data {
                contents.replace(text(t).as_str().into());
            }
        }
        for e in &self.elements {
            if let NodeData::Element { attrs: list, .. } = &e.element.data {
                list.replace(attrs(e).clone());
            }
        }
    }
}

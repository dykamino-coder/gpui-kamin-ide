//! Записи мутаций DOM: единственный вход правки ревизии.
//!
//! Запись ссылается на ручку узла `Document::source_dom`. Текст и атрибуты
//! меняются через ту же ручку, что у JS (DOM §4.10; Blink
//! `CharacterData::SetDataAndUpdate`, Servo `CharacterData::SetData`).
//! Запись удаления указывает непосредственного родителя и ребёнка.
//! `Document::update` переводит разметку той же формы в тот же журнал.
//!
//! Пакет хранит прежние значения, позиции детей и родительские связи.
//! Этого достаточно, чтобы вернуть DOM в прежнее состояние при отказе ревизии.

mod attributes;
mod children;
pub(super) use attributes::attribute_records;

use super::apply::ApplyError;
use html5ever::{Attribute, QualName};
use markup5ever_rcdom::{Handle, NodeData};
use std::rc::Rc;

/// Запись мутации исходного DOM ревизии.
#[derive(Clone)]
pub enum Mutation {
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
    removals: Vec<children::Removal>,
    pub(super) texts: Vec<TextChange>,
    pub(super) elements: Vec<AttrChange>,
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
        Self::collect(records, |_| true).ok()
    }

    pub(super) fn in_document(records: &[Mutation], root: &Handle) -> Result<Self, ApplyError> {
        Self::collect(records, |node| within(node, root))
    }

    fn collect(
        records: &[Mutation],
        attached: impl Fn(&Handle) -> bool,
    ) -> Result<Self, ApplyError> {
        let mut batch = Self::default();
        let mut children = children::Requests::default();
        let mut slots = ScalarSlots::default();
        for record in records {
            match record {
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
        batch.removals = children.finish()?;
        let mut targets = batch
            .texts
            .iter()
            .map(|t| &t.node)
            .chain(batch.elements.iter().map(|e| &e.element))
            .chain(batch.removals.iter().map(|r| &r.parent));
        if !targets.all(attached) {
            return Err(ApplyError::Foreign);
        }
        batch.texts.retain(|t| t.old != t.new);
        batch.elements.retain(|e| e.old != e.new);
        Ok(batch)
    }

    pub(super) fn changes_children(&self) -> bool {
        !self.removals.is_empty()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.texts.is_empty() && self.elements.is_empty() && !self.changes_children()
    }

    /// Записать итоговые значения в исходный DOM.
    pub(super) fn write(&self) {
        self.store(|t| &t.new, |e| &e.new);
        for removal in &self.removals {
            removal.write();
        }
    }

    /// Вернуть прежние значения: пакет отвергнут.
    pub(super) fn undo(&self) {
        for removal in &self.removals {
            removal.undo();
        }
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

/// Узел принадлежит DOM `root` и стоит под элементами HTML вне шаблонов и
/// чужого содержимого: у такого узла атрибуты и текст не часть формы
/// (`shape`). Текст требует родителя-элемента.
pub(super) fn open(node: &Handle, root: &Handle) -> bool {
    let mut at = node.clone();
    let mut first = true;
    loop {
        match &at.data {
            NodeData::Document => return Rc::ptr_eq(&at, root) && !first,
            NodeData::Element { name, .. } => {
                if name.ns != html5ever::ns!(html) || &*name.local == "template" {
                    return false;
                }
            }
            NodeData::Text { .. } if first => {}
            _ => return false,
        }
        let Some(parent) = parent(&at) else {
            return false;
        };
        if first
            && matches!(at.data, NodeData::Text { .. })
            && !matches!(parent.data, NodeData::Element { .. })
        {
            return false;
        }
        first = false;
        at = parent;
    }
}

fn parent(node: &Handle) -> Option<Handle> {
    let weak = node.parent.take();
    let parent = weak
        .as_ref()
        .and_then(markup5ever_rcdom::WeakHandle::upgrade);
    node.parent.set(weak);
    parent
}

fn within(node: &Handle, root: &Handle) -> bool {
    let mut at = Some(node.clone());
    while let Some(node) = at {
        if Rc::ptr_eq(&node, root) {
            return true;
        }
        at = parent(&node);
    }
    false
}

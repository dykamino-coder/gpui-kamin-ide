//! Canonical DOM mutations share one fallible implementation with TreeSink.

use super::{parent_index, remove, reserve, Admission};
use crate::{append, Attribute, Handle, Node, NodeData, QualName, RcDom};
use markup5ever::interface::tree_builder::{ElementFlags, NodeOrText};
use std::{borrow::Cow, cell::RefCell, mem, rc::Rc};
use tendril::StrTendril;

fn append_text<A: Admission>(node: &Handle, text: &str, policy: &A) -> Result<bool, A::Error> {
    let NodeData::Text { contents } = &node.data else {
        return Ok(false);
    };
    let mut contents = contents.borrow_mut();
    let size = contents
        .len()
        .checked_add(text.len())
        .ok_or_else(|| policy.overflow())?;
    policy.buffer(&mut contents, text.len())?;
    let payload = policy.replace_payload(size, 0, &node.payload_lease.borrow())?;
    contents.push_slice(text);
    *node.payload_lease.borrow_mut() = payload;
    Ok(true)
}

impl RcDom {
    pub fn try_create_element<A: Admission>(
        &self,
        name: QualName,
        attrs: Vec<Attribute>,
        flags: ElementFlags,
        policy: &A,
    ) -> Result<Handle, A::Error> {
        let mut owned_attrs = Vec::new();
        let mut attrs_lease = None;
        reserve(&mut owned_attrs, &mut attrs_lease, attrs.len(), policy)?;
        owned_attrs.extend(attrs);
        let template = if flags.template {
            Some(Node::try_new(NodeData::Document, None, policy)?)
        } else {
            None
        };
        Node::try_new(
            NodeData::Element {
                name,
                attrs: RefCell::new(owned_attrs),
                template_contents: RefCell::new(template),
                mathml_annotation_xml_integration_point: flags
                    .mathml_annotation_xml_integration_point,
            },
            attrs_lease,
            policy,
        )
    }

    pub fn try_create_comment<A: Admission>(
        &self,
        contents: StrTendril,
        policy: &A,
    ) -> Result<Handle, A::Error> {
        Node::try_new(NodeData::Comment { contents }, None, policy)
    }

    pub fn try_create_pi<A: Admission>(
        &self,
        target: StrTendril,
        contents: StrTendril,
        policy: &A,
    ) -> Result<Handle, A::Error> {
        Node::try_new(
            NodeData::ProcessingInstruction { target, contents },
            None,
            policy,
        )
    }

    pub fn try_append<A: Admission>(
        &self,
        parent: &Handle,
        child: NodeOrText<Handle>,
        policy: &A,
    ) -> Result<(), A::Error> {
        if let NodeOrText::AppendText(text) = &child {
            if let Some(previous) = parent.children.borrow().last() {
                if append_text(previous, text, policy)? {
                    return Ok(());
                }
            }
        }
        let child = match child {
            NodeOrText::AppendText(text) => Node::try_new(
                NodeData::Text {
                    contents: RefCell::new(text),
                },
                None,
                policy,
            )?,
            NodeOrText::AppendNode(node) => node,
        };
        parent.reserve_children(1, policy)?;
        append(parent, child);
        Ok(())
    }

    pub fn try_before_sibling<A: Admission>(
        &self,
        sibling: &Handle,
        child: NodeOrText<Handle>,
        policy: &A,
    ) -> Result<(), A::Error> {
        let (parent, index) = parent_index(sibling, policy)?.expect("sibling must have a parent");
        if let NodeOrText::AppendText(text) = &child {
            if index > 0 && append_text(&parent.children.borrow()[index - 1], text, policy)? {
                return Ok(());
            }
        }
        let child = match child {
            NodeOrText::AppendText(text) => Node::try_new(
                NodeData::Text {
                    contents: RefCell::new(text),
                },
                None,
                policy,
            )?,
            NodeOrText::AppendNode(node) => node,
        };
        parent.reserve_children(1, policy)?;
        remove(&child, policy)?;
        child.parent.set(Some(crate::WeakHandle::new(&parent)));
        parent.children.borrow_mut().insert(index, child);
        Ok(())
    }

    pub fn try_foster_parent<A: Admission>(
        &self,
        element: &Handle,
        previous: &Handle,
        child: NodeOrText<Handle>,
        policy: &A,
    ) -> Result<(), A::Error> {
        if element.parent().is_some() {
            self.try_before_sibling(element, child, policy)
        } else {
            self.try_append(previous, child, policy)
        }
    }

    pub fn try_doctype<A: Admission>(
        &self,
        name: StrTendril,
        public_id: StrTendril,
        system_id: StrTendril,
        policy: &A,
    ) -> Result<(), A::Error> {
        let node = Node::try_new(
            NodeData::Doctype {
                name,
                public_id,
                system_id,
            },
            None,
            policy,
        )?;
        self.try_append(&self.document, NodeOrText::AppendNode(node), policy)
    }

    pub fn try_attributes<A: Admission>(
        &self,
        target: &Handle,
        attrs: Vec<Attribute>,
        policy: &A,
    ) -> Result<(), A::Error> {
        let NodeData::Element {
            attrs: existing, ..
        } = &target.data
        else {
            panic!("not an element");
        };
        let mut existing = existing.borrow_mut();
        let initial = existing.len();
        let mut bytes = 0usize;
        let mut count = 0usize;
        let duplicate = |attr: &Attribute, previous: &[Attribute]| -> Result<bool, A::Error> {
            for old in previous {
                policy.visit()?;
                if old.name == attr.name {
                    return Ok(true);
                }
            }
            Ok(false)
        };
        for attr in existing.iter().chain(&attrs) {
            policy.visit()?;
            if count >= initial && duplicate(attr, &existing[..initial])? {
                continue;
            }
            bytes = bytes
                .checked_add(attr.name.local.len())
                .and_then(|n| n.checked_add(attr.name.ns.len()))
                .and_then(|n| {
                    n.checked_add(attr.name.prefix.as_ref().map_or(0, |prefix| prefix.len()))
                })
                .and_then(|n| n.checked_add(attr.value.len()))
                .ok_or_else(|| policy.overflow())?;
            count = count.checked_add(1).ok_or_else(|| policy.overflow())?;
        }
        let payload = policy.replace_payload(0, bytes, &target.payload_lease.borrow())?;
        reserve(
            &mut existing,
            &mut target.attrs_lease.borrow_mut(),
            count - initial,
            policy,
        )?;
        for attr in attrs {
            if !duplicate(&attr, &existing[..initial])? {
                existing.push(attr);
            }
        }
        *target.payload_lease.borrow_mut() = payload;
        Ok(())
    }

    pub fn try_remove<A: Admission>(&self, node: &Handle, policy: &A) -> Result<(), A::Error> {
        remove(node, policy)
    }

    pub fn try_reparent<A: Admission>(
        &self,
        node: &Handle,
        parent: &Handle,
        policy: &A,
    ) -> Result<(), A::Error> {
        let mut children = node.children.borrow_mut();
        parent.reserve_children(children.len(), policy)?;
        for child in children.iter() {
            policy.visit()?;
            let previous = child.parent.replace(Some(crate::WeakHandle::new(parent)));
            assert!(Rc::ptr_eq(
                node,
                &previous.unwrap().upgrade().expect("dangling weak")
            ));
        }
        parent
            .children
            .borrow_mut()
            .extend(mem::take(&mut *children));
        node.children_lease.borrow_mut().take();
        Ok(())
    }

    pub fn try_parse_error<A: Admission>(
        &self,
        message: Cow<'static, str>,
        policy: &A,
    ) -> Result<(), A::Error> {
        reserve(
            &mut self.errors.borrow_mut(),
            &mut self.errors_lease.borrow_mut(),
            1,
            policy,
        )?;
        self.errors.borrow_mut().push(message);
        Ok(())
    }
}

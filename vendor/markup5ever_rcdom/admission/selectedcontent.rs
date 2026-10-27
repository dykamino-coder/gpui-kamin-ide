//! Selectedcontent cloning is iterative and propagates every storage refusal.

use super::{reserve, Admission, Work};
use crate::{append, Handle, Node, NodeData, RcDom};
use std::{cell::RefCell, mem, rc::Rc};
use xml5ever::{interface::ElemName, local_name};

impl Node {
    pub(crate) fn try_nearest_select<A: Admission>(
        &self,
        policy: &A,
    ) -> Result<Option<Handle>, A::Error> {
        let mut in_group = false;
        let mut current = self.parent().and_then(|parent| parent.upgrade());
        while let Some(node) = current {
            policy.visit()?;
            if let NodeData::Element { name, .. } = &node.data {
                if matches!(
                    name.local_name(),
                    &local_name!("datalist") | &local_name!("hr") | &local_name!("option")
                ) {
                    return Ok(None);
                }
                if name.local_name() == &local_name!("optgroup") {
                    if in_group {
                        return Ok(None);
                    }
                    in_group = true;
                }
                if name.local_name() == &local_name!("select") {
                    return Ok(Some(node));
                }
            }
            current = node.parent().and_then(|parent| parent.upgrade());
        }
        Ok(None)
    }

    pub(crate) fn try_selectedcontent<A: Admission>(
        &self,
        policy: &A,
    ) -> Result<Option<Handle>, A::Error> {
        let NodeData::Element { attrs, .. } = &self.data else {
            panic!("not an element");
        };
        for attr in attrs.borrow().iter() {
            policy.visit()?;
            if attr.name.local == local_name!("multiple") {
                return Ok(None);
            }
        }
        let mut remaining = Work::new();
        for child in self.children.borrow().iter().rev() {
            policy.visit()?;
            remaining.push(child.clone(), policy)?;
        }
        while let Some(node) = remaining.pop() {
            policy.visit()?;
            if let NodeData::Element { name, .. } = &node.data {
                if name.local_name() == &local_name!("selectedcontent") {
                    return Ok(Some(node));
                }
            }
            for child in node.children.borrow().iter().rev() {
                policy.visit()?;
                remaining.push(child.clone(), policy)?;
            }
        }
        Ok(None)
    }

    fn try_clone_shallow<A: Admission>(&self, policy: &A) -> Result<Handle, A::Error> {
        let mut attrs_lease = None;
        let data = match &self.data {
            NodeData::Element {
                name,
                attrs,
                mathml_annotation_xml_integration_point,
                ..
            } => {
                let mut copied = Vec::new();
                reserve(&mut copied, &mut attrs_lease, attrs.borrow().len(), policy)?;
                policy.work(attrs.borrow().len())?;
                copied.extend_from_slice(&attrs.borrow());
                NodeData::Element {
                    name: name.clone(),
                    attrs: RefCell::new(copied),
                    template_contents: RefCell::new(None),
                    mathml_annotation_xml_integration_point:
                        *mathml_annotation_xml_integration_point,
                }
            }
            // Tendril clones retain their physical storage owner's lease.
            other => other.clone(),
        };
        Node::try_new(data, attrs_lease, policy)
    }

    fn try_clone_subtree<A: Admission>(self: &Rc<Self>, policy: &A) -> Result<Handle, A::Error> {
        let root = self.try_clone_shallow(policy)?;
        let mut remaining = Work::new();
        remaining.push((self.clone(), root.clone()), policy)?;
        while let Some((source, target)) = remaining.pop() {
            policy.visit()?;
            for child in source.children.borrow().iter() {
                policy.visit()?;
                let copied = child.try_clone_shallow(policy)?;
                target.reserve_children(1, policy)?;
                append(&target, copied.clone());
                remaining.push((child.clone(), copied), policy)?;
            }
            if let (
                NodeData::Element {
                    template_contents: source,
                    ..
                },
                NodeData::Element {
                    template_contents: target,
                    ..
                },
            ) = (&source.data, &target.data)
            {
                if let Some(fragment) = source.borrow().as_ref() {
                    let copied = fragment.try_clone_shallow(policy)?;
                    *target.borrow_mut() = Some(copied.clone());
                    remaining.push((fragment.clone(), copied), policy)?;
                }
            }
        }
        Ok(root)
    }

    fn try_clone_into<A: Admission>(&self, target: &Handle, policy: &A) -> Result<(), A::Error> {
        let fragment = Node::try_new(NodeData::Document, None, policy)?;
        for child in self.children.borrow().iter() {
            policy.visit()?;
            let copied = child.try_clone_subtree(policy)?;
            fragment.reserve_children(1, policy)?;
            append(&fragment, copied);
        }
        let children = mem::take(&mut *fragment.children.borrow_mut());
        policy.work(children.len())?;
        policy.work(target.children.borrow().len())?;
        for child in &children {
            child.parent.set(Some(crate::WeakHandle::new(target)));
        }
        let previous = mem::replace(&mut *target.children.borrow_mut(), children);
        for child in &previous {
            child.parent.set(None);
        }
        drop(previous);
        *target.children_lease.borrow_mut() = fragment.children_lease.borrow_mut().take();
        Ok(())
    }
}

impl RcDom {
    pub fn try_selectedcontent<A: Admission>(
        &self,
        option: &Handle,
        policy: &A,
    ) -> Result<(), A::Error> {
        let Some(select) = option.try_nearest_select(policy)? else {
            return Ok(());
        };
        if let Some(target) = select.try_selectedcontent(policy)? {
            if option.try_is_selected_option(policy)? {
                option.try_clone_into(&target, policy)?;
            }
        }
        Ok(())
    }

    pub fn try_pop<A: Admission>(&self, node: &Handle, policy: &A) -> Result<(), A::Error> {
        if let NodeData::Element { name, .. } = &node.data {
            if name.local_name() == &local_name!("option") {
                self.try_selectedcontent(node, policy)?;
            }
        }
        Ok(())
    }

    pub fn try_finish<A: Admission>(self, policy: &A) -> Result<Self, A::Error> {
        let mut stack = Work::new();
        stack.push(self.document.clone(), policy)?;
        while let Some(node) = stack.pop() {
            policy.visit()?;
            if let NodeData::Element { name, .. } = &node.data {
                if name.local_name() == &local_name!("select") {
                    if let Some(target) = node.try_selectedcontent(policy)? {
                        if let Some(option) = node.try_selected_option(policy)? {
                            option.try_clone_into(&target, policy)?;
                        } else {
                            let previous = mem::take(&mut *target.children.borrow_mut());
                            policy.work(previous.len())?;
                            for child in &previous {
                                child.parent.set(None);
                            }
                            drop(previous);
                            target.children_lease.borrow_mut().take();
                        }
                    }
                }
            }
            for child in node.children.borrow().iter() {
                policy.visit()?;
                stack.push(child.clone(), policy)?;
            }
        }
        Ok(self)
    }
}

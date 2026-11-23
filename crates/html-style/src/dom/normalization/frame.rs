//! A child list stays inside its owner throughout the common sibling walk.
use super::record::{Recording, Segment};
use crate::budget::admission::{Admission, Error};
use crate::dom::{Children, Element, Node, same_tree::same_box};
use std::rc::Rc;

pub(super) struct Owner {
    pub element: Option<Element>,
    root: Option<Children>,
}
impl Owner {
    pub fn root(nodes: Children) -> Self {
        Self {
            element: None,
            root: Some(nodes),
        }
    }
    pub fn element(element: Element) -> Self {
        Self {
            element: Some(element),
            root: None,
        }
    }
    pub fn nodes(&self) -> &Children {
        match &self.element {
            Some(e) => &e.children,
            None => self.root.as_ref().expect("root nodes"),
        }
    }
    pub fn nodes_mut(&mut self) -> &mut Children {
        match &mut self.element {
            Some(e) => &mut e.children,
            None => self.root.as_mut().expect("root nodes"),
        }
    }
    pub fn into_root(self) -> Children {
        self.root.expect("root owner")
    }
}

pub(super) struct Frame<C> {
    pub owner: Owner,
    pub context: C,
    pub next: usize,
    pub recording: Option<Recording<C>>,
}
impl<C: Clone + PartialEq> Frame<C> {
    pub fn new(
        owner: Owner,
        context: C,
        recorded: bool,
        old: Option<Rc<Segment<C>>>,
        admission: &Admission,
    ) -> Result<Self, Error> {
        let recording = recorded
            .then(|| Recording::new(owner.nodes().clone(), old, admission))
            .transpose()?;
        Ok(Self {
            owner,
            context,
            next: 0,
            recording,
        })
    }
    pub fn previous(&self, input: &Children) -> Option<Rc<Segment<C>>> {
        self.recording.as_ref()?.previous(input)
    }
    pub fn take_element(&mut self) -> Element {
        let nodes = self.owner.nodes_mut();
        if self.recording.is_some() {
            let Node::Element(element) = &nodes[self.next] else {
                unreachable!("checked element")
            };
            return element.clone();
        }
        let Node::Element(element) =
            std::mem::replace(&mut nodes[self.next], Node::Text(Default::default()))
        else {
            unreachable!("checked element")
        };
        element
    }
    pub fn restore(
        &mut self,
        element: Element,
        segment: Option<Rc<Segment<C>>>,
        admission: &Admission,
    ) -> Result<(), Error> {
        if let Some(recording) = &mut self.recording {
            let Node::Element(original) = &self.owner.nodes()[self.next] else {
                unreachable!("retained input slot")
            };
            let changed = element.children.identity() != original.children.identity()
                || !same_box(&element, original);
            if changed {
                self.owner.nodes_mut().edit(admission, 0)?[self.next] = Node::Element(element);
            }
            recording
                .branches
                .push(segment.expect("recorded child"), admission)?;
        } else {
            self.owner.nodes_mut()[self.next] = Node::Element(element);
        }
        Ok(())
    }
}

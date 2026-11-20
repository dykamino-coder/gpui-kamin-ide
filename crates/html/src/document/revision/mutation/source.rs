//! A script-held reference keeps its document identity while the node is detached.
use super::{Address, ApplyError};
use crate::document::{Document, DocumentFailure};
use markup5ever_rcdom::{Handle, NodeData, WeakHandle};
use std::rc::Rc;

#[derive(Clone)]
pub struct SourceNode {
    node: Handle,
    document: WeakHandle,
}

impl SourceNode {
    pub fn handle(&self) -> &Handle {
        &self.node
    }

    pub(super) fn owned_by(&self, root: &Handle) -> bool {
        self.document
            .upgrade()
            .is_some_and(|owner| Rc::ptr_eq(&owner, root))
    }
}

impl Document {
    /// Capture ownership while resolving a connected node for script.
    pub fn source_node(&self, node: &Handle) -> Result<SourceNode, ApplyError> {
        let root = self.source_dom();
        if !belongs(node, &root, &Default::default()) {
            return Err(ApplyError::Foreign);
        }
        Ok(SourceNode {
            node: node.clone(),
            document: WeakHandle::new(&root),
        })
    }

    /// Allocate a detached element in the same key space and budget as the parser.
    pub fn create_element(&self, name: html5ever::QualName) -> Result<SourceNode, DocumentFailure> {
        let source = self.state.values.traversal.source();
        let context = source.context().continuing(self.budget().revision());
        let node = source.create_element(name, &context)?;
        Ok(SourceNode {
            node,
            document: WeakHandle::new(&source.dom.document),
        })
    }

    pub fn create_text_node(&self, text: &str) -> Result<SourceNode, DocumentFailure> {
        let source = self.state.values.traversal.source();
        let context = source.context().continuing(self.budget().revision());
        let node = source.create_text(text, &context)?;
        Ok(SourceNode {
            node,
            document: WeakHandle::new(&source.dom.document),
        })
    }
}

pub(super) fn belongs(
    node: &Handle,
    root: &Handle,
    certified: &std::collections::HashSet<Address>,
) -> bool {
    let mut at = Some(node.clone());
    while let Some(node) = at {
        if Rc::ptr_eq(&node, root) || certified.contains(&Rc::as_ptr(&node)) {
            return true;
        }
        at = parent(&node);
    }
    false
}

pub(super) fn parent(node: &Handle) -> Option<Handle> {
    let weak = node.parent.take();
    let parent = weak
        .as_ref()
        .and_then(markup5ever_rcdom::WeakHandle::upgrade);
    node.parent.set(weak);
    parent
}

/// Attribute edits can reach SVG elements. SVG text and template contents stay closed.
pub(in crate::document::revision) fn open(node: &Handle, root: &Handle) -> bool {
    let element = matches!(node.data, NodeData::Element { .. });
    let mut at = node.clone();
    let mut first = true;
    loop {
        match &at.data {
            NodeData::Document => return Rc::ptr_eq(&at, root) && !first,
            NodeData::Element { name, .. } => {
                let svg = element && name.ns == html5ever::ns!(svg);
                if !(name.ns == html5ever::ns!(html) || svg) || &*name.local == "template" {
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

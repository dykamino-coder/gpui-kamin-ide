//! A script-held reference keeps its document identity while the node is detached.
use super::{ApplyError, within};
use crate::document::{Document, DocumentFailure};
use markup5ever_rcdom::{Handle, WeakHandle};
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
        self.document.upgrade().is_some_and(|owner| Rc::ptr_eq(&owner, root))
    }
}

impl Document {
    /// Capture ownership while resolving a connected node for script.
    pub fn source_node(&self, node: &Handle) -> Result<SourceNode, ApplyError> {
        let root = self.source_dom();
        if !within(node, &root) {
            return Err(ApplyError::Foreign);
        }
        Ok(SourceNode { node: node.clone(), document: WeakHandle::new(&root) })
    }

    /// Allocate a detached element in the same key space and budget as the parser.
    pub fn create_element(&self, name: html5ever::QualName) -> Result<SourceNode, DocumentFailure> {
        let source = self.state.values.traversal.source();
        let context = source.context().continuing(self.budget().revision());
        let node = source.create_element(name, &context)?;
        Ok(SourceNode { node, document: WeakHandle::new(&source.dom.document) })
    }

    pub fn create_text_node(&self, text: &str) -> Result<SourceNode, DocumentFailure> {
        let source = self.state.values.traversal.source();
        let context = source.context().continuing(self.budget().revision());
        let node = source.create_text(text, &context)?;
        Ok(SourceNode { node, document: WeakHandle::new(&source.dom.document) })
    }
}

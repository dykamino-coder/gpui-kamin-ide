// Copyright 2014-2017 The html5ever Project Developers. See the
// COPYRIGHT file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! <div class=warning>
//!
//! This crate is **not supported**, nor published by the html5ever project.
//!
//! </div>
//!
//! This crate is built for the express purpose of writing automated tests for the html5ever and xml5ever crates. It is not intended to be a production-quality DOM implementation, and has not been fuzzed or tested against arbitrary, malicious, or nontrivial inputs. No maintenance or support for any such issues will be provided. If you use this DOM implementation in a production, user-facing system, you do so at your own risk.
//!
//! A simple reference-counted DOM.
//!
//! This is sufficient as a static parse tree, but don't build a
//! web browser using it. :)
//!
//! A DOM is a [tree structure] with ordered children that can be represented in an XML-like
//! format. For example, the following graph
//!
//! ```text
//! div
//!  +- "text node"
//!  +- span
//! ```
//! in HTML would be serialized as
//!
//! ```html
//! <div>text node<span></span></div>
//! ```
//!
//! See the [document object model article on wikipedia][dom wiki] for more information.
//!
//! This implementation stores the information associated with each node once, and then hands out
//! refs to children. The nodes themselves are reference-counted to avoid copying - you can create
//! a new ref and then a node will outlive the document. Nodes own their children, but only have
//! weak references to their parents.
//!
//! [tree structure]: https://en.wikipedia.org/wiki/Tree_(data_structure)
//! [dom wiki]: https://en.wikipedia.org/wiki/Document_Object_Model

extern crate markup5ever;
extern crate tendril;

pub mod admission;
mod selection;

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::default::Default;
use std::fmt;
use std::io;
use std::mem;
use std::rc::{Rc, Weak};

use tendril::StrTendril;

use markup5ever::interface::tree_builder;
use markup5ever::interface::tree_builder::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use markup5ever::serialize::TraversalScope;
use markup5ever::serialize::TraversalScope::{ChildrenOnly, IncludeNode};
use markup5ever::serialize::{Serialize, Serializer};
use markup5ever::Attribute;
use markup5ever::ExpandedName;
use markup5ever::QualName;

/// The different kinds of nodes in the DOM.
#[derive(Debug, Clone)]
pub enum NodeData {
    /// The `Document` itself - the root node of a HTML document.
    Document,

    /// A `DOCTYPE` with name, public id, and system id. See
    /// [document type declaration on wikipedia][dtd wiki].
    ///
    /// [dtd wiki]: https://en.wikipedia.org/wiki/Document_type_declaration
    Doctype {
        name: StrTendril,
        public_id: StrTendril,
        system_id: StrTendril,
    },

    /// A text node.
    Text { contents: RefCell<StrTendril> },

    /// A comment.
    Comment { contents: StrTendril },

    /// An element with attributes.
    Element {
        name: QualName,
        attrs: RefCell<Vec<Attribute>>,

        /// For HTML \<template\> elements, the [template contents].
        ///
        /// [template contents]: https://html.spec.whatwg.org/multipage/#template-contents
        template_contents: RefCell<Option<Handle>>,

        /// Whether the node is a [HTML integration point].
        ///
        /// [HTML integration point]: https://html.spec.whatwg.org/multipage/#html-integration-point
        mathml_annotation_xml_integration_point: bool,
    },

    /// A Processing instruction.
    ProcessingInstruction {
        target: StrTendril,
        contents: StrTendril,
    },
}

/// A DOM node.
pub struct Node {
    /// Parent node.
    pub parent: Cell<Option<WeakHandle>>,
    /// Child nodes of this node.
    pub children: RefCell<Vec<Handle>>,
    /// Represents this node's data.
    pub data: NodeData,
    // Used only while destroying uniquely owned nodes, without allocating.
    drop_next: RefCell<Option<Handle>>,
    // Storage owners follow the data they cover in declaration/drop order.
    children_lease: RefCell<admission::Lease>,
    attrs_lease: RefCell<admission::Lease>,
    payload_lease: RefCell<admission::Lease>,
    _logical_node: admission::Lease,
    _node_lease: admission::Lease,
}

impl Node {
    /// Create a new node from its contents
    pub fn new(data: NodeData) -> Rc<Self> {
        Rc::new(Node {
            data,
            parent: Cell::new(None),
            children: RefCell::new(Vec::new()),
            drop_next: RefCell::new(None),
            children_lease: RefCell::new(None),
            attrs_lease: RefCell::new(None),
            payload_lease: RefCell::new(None),
            _logical_node: None,
            _node_lease: None,
        })
    }

    fn parent(&self) -> Option<WeakHandle> {
        let parent = self.parent.take();
        self.parent.set(parent.clone());
        parent
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        // Shared subtrees belong to their remaining owners. Only unique nodes
        // may lend their intrusive link to this allocation-free destruction.
        fn enqueue(node: Handle, pending: &mut Option<Handle>) {
            if Rc::strong_count(&node) == 1 {
                *node.drop_next.borrow_mut() = pending.take();
                *pending = Some(node);
            }
        }
        fn detach(node: &Node, pending: &mut Option<Handle>) {
            for child in mem::take(&mut *node.children.borrow_mut()) {
                enqueue(child, pending);
            }
            if let NodeData::Element {
                template_contents, ..
            } = &node.data
            {
                if let Some(fragment) = template_contents.borrow_mut().take() {
                    enqueue(fragment, pending);
                }
            }
        }
        let mut pending = None;
        detach(self, &mut pending);
        while let Some(node) = pending.take() {
            pending = node.drop_next.borrow_mut().take();
            detach(&node, &mut pending);
            // The nested Drop sees an empty node and has constant stack cost.
            drop(node);
        }
    }
}

#[cfg(test)]
#[path = "drop_tests.rs"]
mod drop_tests;

impl fmt::Debug for Node {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Node")
            .field("data", &self.data)
            .field("children", &self.children)
            .finish()
    }
}

/// Reference to a DOM node.
pub type Handle = Rc<Node>;

/// Weak reference to a DOM node, used for parent pointers.
#[derive(Clone)]
pub struct WeakHandle {
    weak: Weak<Node>,
    // Rc storage is freed after the final weak handle, not the final strong one.
    _allocation: admission::Lease,
}

impl WeakHandle {
    fn new(node: &Handle) -> Self {
        Self {
            weak: Rc::downgrade(node),
            _allocation: node._node_lease.clone(),
        }
    }
    pub fn upgrade(&self) -> Option<Handle> {
        self.weak.upgrade()
    }
}

/// Append a parentless node to another nodes' children
fn append(new_parent: &Handle, child: Handle) {
    let previous_parent = child.parent.replace(Some(WeakHandle::new(new_parent)));
    // Invariant: child cannot have existing parent
    assert!(previous_parent.is_none());
    new_parent.children.borrow_mut().push(child);
}

/// If the node has a parent, get it and this node's position in its children
fn remove_from_parent(target: &Handle) {
    admission::infallible(admission::remove(target, &admission::Unbounded));
}

/// The DOM itself; the result of parsing.
pub struct RcDom {
    /// The `Document` itself.
    pub document: Handle,

    /// Errors that occurred during parsing.
    pub errors: RefCell<Vec<Cow<'static, str>>>,

    /// The document's quirks mode.
    pub quirks_mode: Cell<QuirksMode>,
    errors_lease: RefCell<admission::Lease>,
}

impl TreeSink for RcDom {
    type Output = Self;
    fn finish(self) -> Self {
        admission::infallible(self.try_finish(&admission::Unbounded))
    }

    type Handle = Handle;

    type ElemName<'a>
        = ExpandedName<'a>
    where
        Self: 'a;

    fn parse_error(&self, msg: Cow<'static, str>) {
        admission::infallible(self.try_parse_error(msg, &admission::Unbounded))
    }

    fn get_document(&self) -> Handle {
        self.document.clone()
    }

    fn get_template_contents(&self, target: &Handle) -> Handle {
        if let NodeData::Element {
            ref template_contents,
            ..
        } = target.data
        {
            template_contents
                .borrow()
                .as_ref()
                .expect("not a template element!")
                .clone()
        } else {
            panic!("not a template element!")
        }
    }

    fn set_quirks_mode(&self, mode: QuirksMode) {
        self.quirks_mode.set(mode);
    }

    fn same_node(&self, x: &Handle, y: &Handle) -> bool {
        Rc::ptr_eq(x, y)
    }

    fn elem_name<'a>(&self, target: &'a Handle) -> ExpandedName<'a> {
        match target.data {
            NodeData::Element { ref name, .. } => name.expanded(),
            _ => panic!("not an element!"),
        }
    }

    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> Handle {
        admission::infallible(self.try_create_element(name, attrs, flags, &admission::Unbounded))
    }

    fn create_comment(&self, text: StrTendril) -> Handle {
        admission::infallible(self.try_create_comment(text, &admission::Unbounded))
    }

    fn create_pi(&self, target: StrTendril, data: StrTendril) -> Handle {
        admission::infallible(self.try_create_pi(target, data, &admission::Unbounded))
    }

    fn append(&self, parent: &Handle, child: NodeOrText<Handle>) {
        admission::infallible(self.try_append(parent, child, &admission::Unbounded))
    }

    fn append_before_sibling(&self, sibling: &Handle, child: NodeOrText<Handle>) {
        admission::infallible(self.try_before_sibling(sibling, child, &admission::Unbounded))
    }

    fn append_based_on_parent_node(
        &self,
        element: &Self::Handle,
        prev_element: &Self::Handle,
        child: NodeOrText<Self::Handle>,
    ) {
        admission::infallible(self.try_foster_parent(
            element,
            prev_element,
            child,
            &admission::Unbounded,
        ))
    }

    fn append_doctype_to_document(
        &self,
        name: StrTendril,
        public_id: StrTendril,
        system_id: StrTendril,
    ) {
        admission::infallible(self.try_doctype(name, public_id, system_id, &admission::Unbounded))
    }

    fn add_attrs_if_missing(&self, target: &Handle, attrs: Vec<Attribute>) {
        admission::infallible(self.try_attributes(target, attrs, &admission::Unbounded))
    }

    fn remove_from_parent(&self, target: &Handle) {
        remove_from_parent(target);
    }

    fn reparent_children(&self, node: &Handle, new_parent: &Handle) {
        admission::infallible(self.try_reparent(node, new_parent, &admission::Unbounded))
    }

    fn is_mathml_annotation_xml_integration_point(&self, target: &Handle) -> bool {
        if let NodeData::Element {
            mathml_annotation_xml_integration_point,
            ..
        } = target.data
        {
            mathml_annotation_xml_integration_point
        } else {
            panic!("not an element!")
        }
    }

    /// KaminIDE patch: снятие узла со стека открытых элементов.
    ///
    /// Спецификация требует переносить содержимое выбранного `<option>` в
    /// `<selectedcontent>` ИМЕННО в этот момент, а разборщик зовёт крючок
    /// только на одном из путей (закрывающий тег в теле). Пункт, снятый на
    /// конце ввода или следующим пунктом, оставался непереехавшим — набор
    /// html5lib ловит это четырьмя случаями настраиваемого `<select>`.
    fn pop(&self, node: &Self::Handle) {
        admission::infallible(self.try_pop(node, &admission::Unbounded))
    }

    fn maybe_clone_an_option_into_selectedcontent(&self, option: &Self::Handle) {
        admission::infallible(self.try_selectedcontent(option, &admission::Unbounded))
    }
}

impl Default for RcDom {
    fn default() -> RcDom {
        RcDom {
            document: Node::new(NodeData::Document),
            errors: Default::default(),
            quirks_mode: Cell::new(tree_builder::NoQuirks),
            errors_lease: RefCell::new(None),
        }
    }
}

enum SerializeOp {
    Open(Handle),
    Close(QualName),
}

pub struct SerializableHandle(Handle);

impl From<Handle> for SerializableHandle {
    fn from(h: Handle) -> SerializableHandle {
        SerializableHandle(h)
    }
}

impl Serialize for SerializableHandle {
    fn serialize<S>(&self, serializer: &mut S, traversal_scope: TraversalScope) -> io::Result<()>
    where
        S: Serializer,
    {
        let mut ops = VecDeque::new();
        match traversal_scope {
            IncludeNode => ops.push_back(SerializeOp::Open(self.0.clone())),
            ChildrenOnly(_) => ops.extend(
                self.0
                    .children
                    .borrow()
                    .iter()
                    .map(|h| SerializeOp::Open(h.clone())),
            ),
        }

        while let Some(op) = ops.pop_front() {
            match op {
                SerializeOp::Open(handle) => match handle.data {
                    NodeData::Element {
                        ref name,
                        ref attrs,
                        ..
                    } => {
                        serializer.start_elem(
                            name.clone(),
                            attrs.borrow().iter().map(|at| (&at.name, &at.value[..])),
                        )?;

                        ops.reserve(1 + handle.children.borrow().len());
                        ops.push_front(SerializeOp::Close(name.clone()));

                        for child in handle.children.borrow().iter().rev() {
                            ops.push_front(SerializeOp::Open(child.clone()));
                        }
                    }

                    NodeData::Doctype { ref name, .. } => serializer.write_doctype(name)?,

                    NodeData::Text { ref contents } => serializer.write_text(&contents.borrow())?,

                    NodeData::Comment { ref contents } => serializer.write_comment(contents)?,

                    NodeData::ProcessingInstruction {
                        ref target,
                        ref contents,
                    } => serializer.write_processing_instruction(target, contents)?,

                    NodeData::Document => panic!("Can't serialize Document node itself"),
                },

                SerializeOp::Close(name) => {
                    serializer.end_elem(name)?;
                }
            }
        }

        Ok(())
    }
}

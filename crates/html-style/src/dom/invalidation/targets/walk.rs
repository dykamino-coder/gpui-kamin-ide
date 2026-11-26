//! The moves of an invalidation path over the source DOM (`Move`): the
//! elements one step reaches from a set of elements, and the feature test of
//! a path's subject.

use super::super::Kind;
use super::Move;
use markup5ever_rcdom::{Handle, NodeData};
use std::rc::Rc;

impl super::Features {
    /// Whether a rule reads a changed name in a compound whose element is a
    /// sibling of the subject's ancestors or of the subject (`+`, `~`,
    /// `:nth-child(of S)`, `:has()`, or an argument this map does not trace):
    /// only then does a mutation change what a parent's census of its
    /// children answers (Blink builds sibling invalidation sets only for
    /// such features, `rule_invalidation_data.cc`).
    pub fn reads_siblings(&self) -> bool {
        self.0.iter().flat_map(|(_, path, _)| path).any(|step| {
            matches!(
                step,
                Move::NextSibling
                    | Move::LaterSiblings
                    | Move::Siblings
                    | Move::HasAnchors
                    | Move::Document
            )
        })
    }
}

/// The elements one `step` reaches from `set`, each once.
pub(super) fn apply(step: Move, set: &[Handle]) -> Vec<Handle> {
    let mut out = Targets::default();
    reach(step, set, &mut |n| out.add(n));
    out.list
}

/// Calls `each` with every element one `step` reaches from `set` (an element
/// reached from two members of `set` twice), collecting nothing: the last
/// step of a path tests the subject feature as it goes, as Blink's descendant
/// invalidation checks each element of the subtree in place.
pub(super) fn reach(step: Move, set: &[Handle], each: &mut dyn FnMut(&Handle)) {
    for node in set {
        match step {
            Move::Children => children(node).for_each(|c| each(&c)),
            Move::Descendants => descendants(node, each),
            Move::NextSibling => {
                if let Some(next) = siblings(node).skip_while(|s| !Rc::ptr_eq(s, node)).nth(1) {
                    each(&next);
                }
            }
            Move::LaterSiblings => siblings(node)
                .skip_while(|s| !Rc::ptr_eq(s, node))
                .skip(1)
                .for_each(|s| each(&s)),
            Move::Siblings => siblings(node).for_each(|s| each(&s)),
            Move::HasAnchors => {
                let mut at = Some(node.clone());
                while let Some(n) = at {
                    siblings(&n)
                        .take_while(|s| !Rc::ptr_eq(s, &n))
                        .for_each(|s| each(&s));
                    at = parent(&n);
                    if let Some(p) = at.as_ref().filter(|p| is_element(p)) {
                        each(p);
                    }
                }
            }
            Move::Document => {
                let mut root = node.clone();
                while let Some(p) = parent(&root) {
                    root = p;
                }
                descendants(&root, each);
            }
        }
    }
}
fn is_element(node: &Handle) -> bool {
    matches!(node.data, NodeData::Element { .. })
}

fn parent(node: &Handle) -> Option<Handle> {
    let weak = node.parent.take();
    let parent = weak.as_ref().and_then(|w| w.upgrade());
    node.parent.set(weak);
    parent
}

/// The element children of `node`.
fn children(node: &Handle) -> impl Iterator<Item = Handle> {
    let kids: Vec<Handle> = node
        .children
        .borrow()
        .iter()
        .filter(|c| is_element(c))
        .cloned()
        .collect();
    kids.into_iter()
}

/// The element siblings of `node`, itself included, in tree order.
fn siblings(node: &Handle) -> impl Iterator<Item = Handle> {
    let kids: Vec<Handle> = match parent(node) {
        Some(p) => children(&p).collect(),
        None => vec![node.clone()],
    };
    kids.into_iter()
}

/// The element descendants of `node` in tree order, not inside a `template`
/// (its content is no part of the rendered tree).
fn descendants(node: &Handle, each: &mut dyn FnMut(&Handle)) {
    let mut stack: Vec<Handle> = node
        .children
        .borrow()
        .iter()
        .rev()
        .filter(|c| is_element(c))
        .cloned()
        .collect();
    while let Some(n) = stack.pop() {
        let NodeData::Element { name, .. } = &n.data else {
            continue;
        };
        each(&n);
        if &*name.local != "template" {
            stack.extend(
                n.children
                    .borrow()
                    .iter()
                    .rev()
                    .filter(|c| is_element(c))
                    .cloned(),
            );
        }
    }
}
/// The element `node` has the feature.
pub(super) fn has(kind: Kind, feature: &str, node: &Handle) -> bool {
    let NodeData::Element {
        name: tag, attrs, ..
    } = &node.data
    else {
        return false;
    };
    let attrs = attrs.borrow();
    let value = |n: &str| {
        attrs
            .iter()
            .find(|a| (*a.name.local).eq_ignore_ascii_case(n))
            .map(|a| &*a.value)
    };
    match kind {
        Kind::Tag => (*tag.local).eq_ignore_ascii_case(feature),
        Kind::Attr => value(feature).is_some(),
        Kind::Id => value("id").is_some_and(|v| v.eq_ignore_ascii_case(feature)),
        Kind::Class => value("class").is_some_and(|v| {
            v.split_ascii_whitespace()
                .any(|t| t.eq_ignore_ascii_case(feature))
        }),
    }
}

/// Elements in insertion order without repeats.
#[derive(Default)]
pub(super) struct Targets {
    pub(super) list: Vec<Handle>,
    seen: std::collections::HashSet<usize>,
}

impl Targets {
    pub(super) fn add(&mut self, node: &Handle) {
        if self.seen.insert(Rc::as_ptr(node) as usize) {
            self.list.push(node.clone());
        }
    }
}

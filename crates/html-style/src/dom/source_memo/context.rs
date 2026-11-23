//! Shared cascade inputs detached from traversal caches and source handles.
use super::NodeKey;
use crate::dom::shadow::Scope;
use crate::style::cascade::sharing::Environment;
use crate::style::computed::ContentItem;
use crate::style::css::VarMap;
use crate::style::generated::{counter_style, counters::Counters};
use std::rc::Rc;

#[derive(Clone, PartialEq)]
pub struct GeneratedContext {
    pub counters: [Option<String>; 3],
    pub counter_style: counter_style::Context,
    pub content: Option<Vec<ContentItem>>,
    pub content_none: Option<bool>,
    pub language: Option<Rc<str>>,
    pub target_group: bool,
}

/// A parent spine shares inputs; it never copies the complete ancestor vector.
pub struct CascadeContext {
    pub parent: Option<Rc<Self>>,
    pub node: NodeKey,
    pub tag: String,
    pub attrs: Vec<(String, String)>,
    pub generated: GeneratedContext,
}

impl Drop for CascadeContext {
    fn drop(&mut self) {
        // Releasing a deep, uniquely held spine must not recurse on the thread stack.
        let mut parent = self.parent.take();
        while let Some(link) = parent {
            match Rc::try_unwrap(link) {
                Ok(mut unique) => parent = unique.parent.take(),
                Err(_) => break,
            }
        }
    }
}

impl PartialEq for CascadeContext {
    fn eq(&self, other: &Self) -> bool {
        let (mut left, mut right) = (self, other);
        loop {
            if std::ptr::eq(left, right) {
                return true;
            }
            if left.node != right.node
                || left.tag != right.tag
                || left.attrs != right.attrs
                || left.generated != right.generated
            {
                return false;
            }
            match (&left.parent, &right.parent) {
                (Some(a), Some(b)) => (left, right) = (a, b),
                (None, None) => return true,
                _ => return false,
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroupInput {
    pub node: NodeKey,
    pub selected: bool,
}

#[derive(Clone)]
pub struct EntryInputs {
    pub parent: Option<Rc<CascadeContext>>,
    pub scope: Rc<Scope>,
    pub vars: Rc<VarMap>,
    pub preserve: bool,
    pub depth: usize,
    pub(crate) environment: Rc<Environment>,
    pub counters: Counters,
    pub group: Option<GroupInput>,
}

impl PartialEq for EntryInputs {
    fn eq(&self, other: &Self) -> bool {
        // Scope identity is sufficient only while the retained rule index is current.
        // A different index must enter cascade even when its text happened to match.
        Rc::ptr_eq(&self.scope, &other.scope)
            && self.parent == other.parent
            && self.vars == other.vars
            && self.preserve == other.preserve
            && self.depth == other.depth
            && self.environment == other.environment
            && self.counters == other.counters
            && self.group == other.group
    }
}

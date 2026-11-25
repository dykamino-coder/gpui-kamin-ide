//! The part of a compound a batch cannot change (`Admits`): its type, id,
//! classes and attribute names that name no changed name, and the
//! alternatives of its `:is()`/`:where()`. A mutated element that fails it
//! matches the compound neither before nor after the batch, so no rule
//! reached through that compound changes its match for the element.
//!
//! Stylo matches each dependency's compound against the element and its
//! snapshot and invalidates only when the result differs
//! (`invalidation/element/invalidator.rs`, `state_and_attributes.rs`); Blink
//! keys invalidation sets by the changed feature alone. This keeps the half
//! that needs no matcher: a necessary condition of a match in either state,
//! so it never drops a path a rebuild would restyle.

use super::super::Changed;
use crate::style::css::{PseudoKind, Selector};
use markup5ever_rcdom::{Handle, NodeData};

/// What every match of a compound requires of its element in both states of
/// a batch.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Admits {
    tag: Option<String>,
    id: Option<String>,
    classes: Vec<String>,
    attrs: Vec<String>,
    /// One `:is()`/`:where()` each: an element admitted by none of its
    /// alternatives' subjects is not admitted.
    any_of: Vec<Vec<Admits>>,
}

impl Admits {
    /// The unchanging part of the compound `sel` under the batch `changed`.
    pub(super) fn of(sel: &Selector, changed: &Changed) -> Self {
        let among = |names: &[String], n: &str| names.iter().any(|c| c.eq_ignore_ascii_case(n));
        let kept = |n: &&String| !among(&changed.classes, n);
        let tag = sel.tag.clone().filter(|t| t != "*");
        let id = (sel.id.clone()).filter(|_| !among(&changed.attrs, "id"));
        let classes = match among(&changed.attrs, "class") {
            true => sel.classes.iter().filter(kept).cloned().collect(),
            false => sel.classes.clone(),
        };
        let attrs = (sel.attrs.iter())
            .map(|a| &a.name)
            .filter(|n| !among(&changed.attrs, n))
            .cloned()
            .collect();
        let any_of = (sel.pseudo.iter().chain(&sel.also))
            .filter_map(|raw| sel.prepared(raw))
            .filter(|arg| matches!(arg.kind, PseudoKind::Is) && !arg.list.is_empty())
            .map(|arg| arg.list.iter().map(|s| Self::of(s, changed)).collect())
            .collect();
        Self {
            tag,
            id,
            classes,
            attrs,
            any_of,
        }
    }

    /// The element `node` passes the unchanging part, ignoring ASCII case
    /// (a looser test admits more and drops nothing a match needs).
    pub(super) fn admits(&self, node: &Handle) -> bool {
        let NodeData::Element { name, attrs, .. } = &node.data else {
            return false;
        };
        let attrs = attrs.borrow();
        let value = |n: &str| {
            (attrs.iter())
                .find(|a| (*a.name.local).eq_ignore_ascii_case(n))
                .map(|a| &*a.value)
        };
        let class = value("class").unwrap_or_default();
        let has_class =
            |c: &String| (class.split_ascii_whitespace()).any(|t| t.eq_ignore_ascii_case(c));
        self.tag
            .as_ref()
            .is_none_or(|t| (*name.local).eq_ignore_ascii_case(t))
            && (self.id.as_ref())
                .is_none_or(|id| value("id").is_some_and(|v| v.eq_ignore_ascii_case(id)))
            && self.classes.iter().all(has_class)
            && self.attrs.iter().all(|a| value(a).is_some())
            && (self.any_of.iter()).all(|alts| alts.iter().any(|a| a.admits(node)))
    }
}

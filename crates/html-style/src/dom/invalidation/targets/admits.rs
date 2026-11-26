//! The simple selectors of a compound a mutated element must pass in one of
//! the two states of a batch (`Admits`): its type, id, classes, attribute
//! conditions and the alternatives of its `:is()`/`:where()`. An element that
//! passes them in neither state matches the compound neither before nor after
//! the batch, so no rule reached through that compound changes its match for
//! the element (`[class*=x] .y` stays as it was when `html` gains a class
//! that holds no `x`).
//!
//! Stylo matches each dependency's compound against the element and its
//! snapshot and invalidates only when the result differs
//! (`invalidation/element/invalidator.rs`, `state_and_attributes.rs`); Blink
//! keys invalidation sets by the changed feature alone. This keeps the half
//! that needs no matcher: a necessary condition of a match in either state,
//! so it never drops a path a rebuild would restyle. Values compare ignoring
//! ASCII case, a looser test than any the matcher applies.

use super::super::Changed;
use crate::style::css::{AttrSel, PseudoKind, Selector};
use markup5ever_rcdom::{Handle, NodeData};

/// A simple selector and whether it reads a name the batch changes.
type Part<T> = (T, bool);

/// What every match of a compound requires of its element.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Admits {
    tag: Option<String>,
    id: Option<Part<String>>,
    classes: Vec<Part<String>>,
    attrs: Vec<Part<AttrSel>>,
    /// One `:is()`/`:where()` each: an element admitted by none of its
    /// alternatives' subjects is not admitted.
    any_of: Vec<Vec<Admits>>,
}

/// The attributes of an element in one state of a batch.
pub(super) type Attrs<'a> = &'a [html5ever::Attribute];

impl Admits {
    /// The simple selectors of the compound `sel`, marked by whether they
    /// read a name of `changed`.
    pub(super) fn of(sel: &Selector, changed: &Changed) -> Self {
        let among = |names: &[String], n: &str| names.iter().any(|c| c.eq_ignore_ascii_case(n));
        let class_changes = among(&changed.attrs, "class");
        let any_of = (sel.pseudo.iter().chain(&sel.also))
            .filter_map(|raw| sel.prepared(raw))
            .filter(|arg| matches!(arg.kind, PseudoKind::Is) && !arg.list.is_empty())
            .map(|arg| arg.list.iter().map(|s| Self::of(s, changed)).collect())
            .collect();
        Self {
            tag: sel.tag.clone().filter(|t| t != "*"),
            id: (sel.id.clone()).map(|id| (id, among(&changed.attrs, "id"))),
            classes: (sel.classes.iter())
                .map(|c| (c.clone(), class_changes && among(&changed.classes, c)))
                .collect(),
            attrs: (sel.attrs.iter())
                .map(|a| (a.clone(), among(&changed.attrs, &a.name)))
                .collect(),
            any_of,
        }
    }

    /// The mutated element `node` passes the compound before the batch (its
    /// attributes now) or after it (`after`); without `after`, the simple
    /// selectors that read a changed name are taken as passing.
    pub(super) fn admits(&self, node: &Handle, after: Option<Attrs<'_>>) -> bool {
        let NodeData::Element { name, attrs, .. } = &node.data else {
            return false;
        };
        let now = attrs.borrow();
        let tag = &*name.local;
        match after {
            Some(after) => self.passes(tag, &now[..], false) || self.passes(tag, after, false),
            None => self.passes(tag, &now[..], true),
        }
    }

    /// The element of `tag` with `attrs` passes; `loose`: the parts that
    /// read a changed name pass.
    fn passes(&self, tag: &str, attrs: Attrs<'_>, loose: bool) -> bool {
        let value = |n: &str| {
            (attrs.iter())
                .find(|a| (*a.name.local).eq_ignore_ascii_case(n))
                .map(|a| &*a.value)
        };
        let class = value("class").unwrap_or_default();
        let has_class =
            |c: &String| (class.split_ascii_whitespace()).any(|t| t.eq_ignore_ascii_case(c));
        let id = |id: &String| value("id").is_some_and(|v| v.eq_ignore_ascii_case(id));
        let attr = |a: &AttrSel| {
            let a = AttrSel {
                ci: true,
                ..a.clone()
            };
            a.matches(value(&a.name))
        };
        self.tag
            .as_ref()
            .is_none_or(|t| tag.eq_ignore_ascii_case(t))
            && self.id.as_ref().is_none_or(|p| held(p, loose, id))
            && self.classes.iter().all(|p| held(p, loose, has_class))
            && self.attrs.iter().all(|p| held(p, loose, attr))
            && (self.any_of.iter()).all(|alts| alts.iter().any(|a| a.passes(tag, attrs, loose)))
    }
}

/// The simple selector `part` holds, or reads a changed name and `loose`.
fn held<T>((part, changed): &Part<T>, loose: bool, test: impl Fn(&T) -> bool) -> bool {
    loose && *changed || test(part)
}

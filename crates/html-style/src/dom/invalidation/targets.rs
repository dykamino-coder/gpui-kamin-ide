//! The elements a batch of class mutations may restyle, found before any walk
//! from the rules that read a changed name.
//!
//! Blink keeps per feature an invalidation set whose kind follows the
//! combinator to the right of the compound naming it: the compound is the
//! rule's subject (`InvalidatesSelf`), or an ancestor of it (a descendant set
//! carrying the subject's features, `invalidation_set.h:462`), or a previous
//! sibling (`SiblingInvalidationSet`, `:473`); Stylo derives the same kinds
//! from the combinator (`invalidation_map.rs:170`). Here the subject and
//! ancestor kinds are followed: the mutated element itself, and its
//! descendants that have the subject feature of a rule naming the change in
//! an ancestor compound. A name inside a sibling compound, a selector
//! argument (`:is()`, `:not()`, `:has()`) or a pseudo-class or -element of a
//! dependent subject is not followed: `None`, any element may restyle.

use super::{Changed, Kind, LIMIT, kind, name, reads};
use crate::style::css::{Rule, Selector};
use markup5ever_rcdom::{Handle, NodeData};

/// Where a selector reads a changed name.
#[derive(Clone, Copy, PartialEq)]
enum Reach {
    /// The subject compound: the element whose names changed.
    Subject,
    /// An ancestor compound only: descendants with the subject feature.
    Descendants,
}

/// The subject features of the rules that read a changed name in an ancestor
/// compound: what Blink keeps as the descendant invalidation set of the name
/// (`invalidation_set.h:462`). An entry without a feature: any descendant.
pub struct Features(Vec<Option<(Kind, String)>>);

/// The descendant features of `rules` for the names of `changed`; `None`: a
/// rule reads a name where this map does not follow, any element may restyle.
pub fn features(rules: &[Rule], changed: &Changed) -> Option<Features> {
    let mut features: Vec<Option<(Kind, String)>> = Vec::new();
    for rule in rules {
        match reach(&rule.sel, changed)? {
            None | Some(Reach::Subject) => {}
            Some(Reach::Descendants) => {
                let feature = kind(&rule.sel).map(|k| (k, name(&rule.sel, k).to_string()));
                if !features.contains(&feature) {
                    features.push(feature);
                }
            }
        }
    }
    Some(Features(features))
}

/// The elements `mutated` and those under them that a rule of `rules` may
/// restyle after the names of `changed` changed on them; `None`: any element
/// may restyle, or more than the invalidation limit.
pub fn targets(rules: &[Rule], changed: &Changed, mutated: &[Handle]) -> Option<Vec<Handle>> {
    features(rules, changed)?.targets(mutated)
}

impl Features {
    /// The elements `mutated` and those under them that have one of the
    /// features; `None`: more than the invalidation limit.
    pub fn targets(&self, mutated: &[Handle]) -> Option<Vec<Handle>> {
        let features = &self.0;
        let mut out: Vec<Handle> = mutated.to_vec();
        if features.is_empty() {
            return Some(out);
        }
        for root in mutated {
            let mut stack: Vec<Handle> = root.children.borrow().clone();
            while let Some(node) = stack.pop() {
                let NodeData::Element {
                    name: tag, attrs, ..
                } = &node.data
                else {
                    continue;
                };
                if &*tag.local == "template" {
                    continue;
                }
                let hit = {
                    let attrs = attrs.borrow();
                    features.iter().any(|f| match f {
                        None => true,
                        Some((kind, feature)) => has(*kind, feature, &tag.local, &attrs),
                    })
                };
                if hit && !out.iter().any(|h| std::rc::Rc::ptr_eq(h, &node)) {
                    if out.len() == mutated.len() + LIMIT {
                        return None;
                    }
                    out.push(node.clone());
                }
                stack.extend(node.children.borrow().iter().cloned());
            }
        }
        Some(out)
    }
}

/// Where `sel` reads a name of `changed`: `Ok(None)` nowhere, `None` (outer)
/// somewhere this map does not follow.
fn reach(sel: &Selector, changed: &Changed) -> Option<Option<Reach>> {
    if !reads(sel, changed) {
        return Some(None);
    }
    let pseudo = sel.pseudo.is_some() || !sel.also.is_empty();
    if pseudo
        || !sel.arguments.is_empty()
        || sel.prev.as_ref().is_some_and(|p| reads(&p.0, changed))
    {
        return None;
    }
    if !sel.ancestor.as_ref().is_some_and(|a| reads(&a.0, changed)) {
        return own(sel, changed).then_some(Some(Reach::Subject));
    }
    // The name is in the ancestor chain: only plain compounds are followed.
    let mut at = sel.ancestor.as_deref();
    while let Some((ancestor, _)) = at {
        let extra = ancestor.pseudo.is_some() || !ancestor.also.is_empty();
        let sibling = ancestor.prev.as_ref().is_some_and(|p| reads(&p.0, changed));
        if extra && reads(ancestor, changed) || sibling || !ancestor.arguments.is_empty() {
            return None;
        }
        at = ancestor.ancestor.as_deref();
    }
    Some(Some(Reach::Descendants))
}

/// The subject compound of `sel` itself names a changed class or attribute.
fn own(sel: &Selector, changed: &Changed) -> bool {
    let among = |names: &[String], n: &str| names.iter().any(|c| c.eq_ignore_ascii_case(n));
    sel.classes.iter().any(|c| among(&changed.classes, c))
        || sel.attrs.iter().any(|a| among(&changed.attrs, &a.name))
}

/// The element of tag `tag` and attributes `attrs` has the feature.
fn has(kind: Kind, feature: &str, tag: &str, attrs: &[html5ever::Attribute]) -> bool {
    let value = |n: &str| {
        attrs
            .iter()
            .find(|a| (*a.name.local).eq_ignore_ascii_case(n))
            .map(|a| &*a.value)
    };
    match kind {
        Kind::Tag => tag.eq_ignore_ascii_case(feature),
        Kind::Attr => value(feature).is_some(),
        Kind::Id => value("id").is_some_and(|v| v.eq_ignore_ascii_case(feature)),
        Kind::Class => value("class").is_some_and(|v| {
            v.split_ascii_whitespace()
                .any(|t| t.eq_ignore_ascii_case(feature))
        }),
    }
}

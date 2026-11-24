//! The elements a batch of class and attribute mutations may restyle, found
//! before any walk from the rules that read a changed name. There is always
//! such a set: a selector this map cannot trace widens it (up to the whole
//! document), it never refuses the targeted restyle.
//!
//! A selector reads a name in some compound. The subject of the rule is then
//! reached from the mutated element by the combinators between that compound
//! and the subject (Selectors-4 §16), each of them one move: a descendant or
//! child combinator moves to descendants or children, `~` and `+` move to
//! later or next siblings. A pseudo-class argument is a selector of its own
//! whose subject is the compound's element: `:is()`, `:where()` and `:not()`
//! add no move (§4); `:nth-child(An+B of S)` moves to every sibling, whose
//! positions it counts (§14.4); `:has(R)` moves to every possible anchor,
//! the ancestors and their earlier siblings (§4.5); `:lang()` and `:dir()`
//! read the attribute on ancestors too and move to descendants (§7). State
//! pseudo-classes and pseudo-elements depend on their own element only. A
//! name in an argument this map does not parse moves to the whole document.
//!
//! Blink keeps the same as invalidation sets: self, descendant
//! (`invalidation_set.h:462`), sibling (`:473`) and `:has()` sets
//! (`rule_invalidation_data.h`); Stylo derives them from the combinators
//! (`invalidation_map.rs:170`). Here the moves of each reading compound are
//! composed into paths, and the subject feature of the rule (its id, class,
//! tag or attribute) filters the elements a path reaches.

use super::{Changed, Kind, kind, mentions, name};
use crate::style::css::{PseudoKind, Rule, Selector};
use markup5ever_rcdom::{Handle, NodeData};
use std::rc::Rc;

/// One step from a set of elements to the elements a selector may reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Move {
    Children,
    Descendants,
    NextSibling,
    LaterSiblings,
    /// Every sibling, the element included (`:nth-child(of S)`).
    Siblings,
    /// The ancestors and the earlier siblings of each of them and of the
    /// element: every element whose `:has()` may see it.
    HasAnchors,
    /// The whole document.
    Document,
}

/// The paths of the rules that read a changed name: the moves from a mutated
/// element to the rule's subject, and the subject feature (`None`: any).
pub struct Features(Vec<(Vec<Move>, Option<(Kind, String)>)>);

/// The paths of `rules` for the names of `changed`.
pub fn features(rules: &[Rule], changed: &Changed) -> Features {
    let mut out: Vec<(Vec<Move>, Option<(Kind, String)>)> = Vec::new();
    for rule in rules {
        let paths = paths(&rule.sel, changed);
        if paths.is_empty() {
            continue;
        }
        let feature = kind(&rule.sel).map(|k| (k, name(&rule.sel, k).to_string()));
        for path in paths {
            // The mutated elements are always targets: an empty path adds nothing.
            if path.is_empty() {
                continue;
            }
            let path = if path.contains(&Move::Document) {
                vec![Move::Document]
            } else {
                path
            };
            let entry = (path, feature.clone());
            if !out.contains(&entry) {
                out.push(entry);
            }
        }
    }
    Features(out)
}

/// The elements `mutated` and those a rule of `rules` may restyle after the
/// names of `changed` changed on them.
pub fn targets(rules: &[Rule], changed: &Changed, mutated: &[Handle]) -> Vec<Handle> {
    features(rules, changed).targets(mutated)
}

impl Features {
    /// The elements `mutated` and those each path reaches from them with the
    /// subject feature of its rule, in that order, each once.
    pub fn targets(&self, mutated: &[Handle]) -> Vec<Handle> {
        let mut out = Targets::default();
        for m in mutated {
            out.add(m);
        }
        for (path, feature) in &self.0 {
            let mut set: Vec<Handle> = mutated.to_vec();
            for &step in path {
                set = apply(step, &set);
            }
            for node in &set {
                if feature.as_ref().is_none_or(|(k, f)| has(*k, f, node)) {
                    out.add(node);
                }
            }
        }
        out.list
    }
}

/// Elements in insertion order without repeats.
#[derive(Default)]
struct Targets {
    list: Vec<Handle>,
    seen: std::collections::HashSet<usize>,
}

impl Targets {
    fn add(&mut self, node: &Handle) {
        if self.seen.insert(Rc::as_ptr(node) as usize) {
            self.list.push(node.clone());
        }
    }
}

/// The moves from an element whose names changed to the subject of `sel`,
/// one path per compound (or argument) that reads a changed name.
fn paths(sel: &Selector, changed: &Changed) -> Vec<Vec<Move>> {
    let mut out = own(sel, changed);
    if let Some(ancestor) = &sel.ancestor {
        let step = if ancestor.1 {
            Move::Children
        } else {
            Move::Descendants
        };
        out.extend(paths(&ancestor.0, changed).into_iter().map(|mut p| {
            p.push(step);
            p
        }));
    }
    if let Some(prev) = &sel.prev {
        let step = if prev.1 {
            Move::NextSibling
        } else {
            Move::LaterSiblings
        };
        out.extend(paths(&prev.0, changed).into_iter().map(|mut p| {
            p.push(step);
            p
        }));
    }
    out
}

/// The paths that end at the element of the compound `sel` itself: its own
/// classes, attributes and id, and the arguments of its pseudo-classes.
fn own(sel: &Selector, changed: &Changed) -> Vec<Vec<Move>> {
    let among = |names: &[String], n: &str| names.iter().any(|c| c.eq_ignore_ascii_case(n));
    let mut out = Vec::new();
    if sel.classes.iter().any(|c| among(&changed.classes, c))
        || sel.attrs.iter().any(|a| among(&changed.attrs, &a.name))
        || sel.id.is_some() && among(&changed.attrs, "id")
    {
        out.push(Vec::new());
    }
    for raw in sel.pseudo.iter().chain(&sel.also) {
        out.extend(pseudo(sel, raw, changed));
    }
    out
}

/// The paths through the pseudo-class `raw` of the compound `sel`.
fn pseudo(sel: &Selector, raw: &str, changed: &Changed) -> Vec<Vec<Move>> {
    let among = |names: &[&str]| {
        names
            .iter()
            .any(|n| changed.attrs.iter().any(|a| a.eq_ignore_ascii_case(n)))
    };
    let text = || {
        raw.contains('\\')
            || changed
                .classes
                .iter()
                .chain(&changed.attrs)
                .any(|n| mentions(raw, n))
    };
    let Some(arg) = sel.prepared(raw) else {
        // Not parsed here (or a pseudo-element): a name in it may be read anywhere.
        return if text() {
            vec![vec![Move::Document]]
        } else {
            Vec::new()
        };
    };
    let inner = |step: Option<Move>| -> Vec<Vec<Move>> {
        arg.list
            .iter()
            .flat_map(|s| paths(s, changed))
            .map(|mut p| {
                p.extend(step);
                p
            })
            .collect()
    };
    // A name the argument's text holds where no parsed selector reads it
    // (a part this parser dropped) may be read anywhere.
    let traced = |paths: Vec<Vec<Move>>| {
        if paths.is_empty() && text() {
            vec![vec![Move::Document]]
        } else {
            paths
        }
    };
    match arg.kind {
        PseudoKind::Not | PseudoKind::Is => traced(inner(None)),
        PseudoKind::NthOf(_) => traced(inner(Some(Move::Siblings))),
        PseudoKind::Has(_) => traced(inner(Some(Move::HasAnchors))),
        // `:checked` reads the attributes its state derives from (`form_state`).
        PseudoKind::Checked => {
            if among(&crate::dom::form_state::READS) {
                vec![Vec::new()]
            } else {
                Vec::new()
            }
        }
        // The language and direction of an element come from its ancestors.
        PseudoKind::Lang | PseudoKind::Dir(_) => {
            if among(&["lang", "dir", "xml:lang"]) || text() {
                vec![Vec::new(), vec![Move::Descendants]]
            } else {
                Vec::new()
            }
        }
        PseudoKind::Structural(_, _)
        | PseudoKind::Link
        | PseudoKind::Visited
        | PseudoKind::TargetCurrent
        | PseudoKind::Root
        | PseudoKind::Empty
        | PseudoKind::UserAction => inner(None),
        // Shadow-tree and unsupported pseudo-classes: anywhere.
        PseudoKind::HasSlotted | PseudoKind::Host | PseudoKind::Other => {
            let mut read = inner(None);
            if !read.is_empty() || text() {
                read = vec![vec![Move::Document]];
            }
            read
        }
    }
}

/// The elements one `step` reaches from `set`.
fn apply(step: Move, set: &[Handle]) -> Vec<Handle> {
    let mut out = Targets::default();
    for node in set {
        match step {
            Move::Children => children(node).for_each(|c| out.add(&c)),
            Move::Descendants => descendants(node, &mut out),
            Move::NextSibling => {
                if let Some(next) = siblings(node).skip_while(|s| !Rc::ptr_eq(s, node)).nth(1) {
                    out.add(&next);
                }
            }
            Move::LaterSiblings => siblings(node)
                .skip_while(|s| !Rc::ptr_eq(s, node))
                .skip(1)
                .for_each(|s| out.add(&s)),
            Move::Siblings => siblings(node).for_each(|s| out.add(&s)),
            Move::HasAnchors => {
                let mut at = Some(node.clone());
                while let Some(n) = at {
                    siblings(&n)
                        .take_while(|s| !Rc::ptr_eq(s, &n))
                        .for_each(|s| out.add(&s));
                    at = parent(&n);
                    if let Some(p) = &at {
                        if is_element(p) {
                            out.add(p);
                        }
                    }
                }
            }
            Move::Document => {
                let mut root = node.clone();
                while let Some(p) = parent(&root) {
                    root = p;
                }
                descendants(&root, &mut out);
            }
        }
    }
    out.list
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

/// The element descendants of `node`, not inside a `template` (its content
/// is no part of the rendered tree).
fn descendants(node: &Handle, out: &mut Targets) {
    let mut stack: Vec<Handle> = children(node).collect();
    stack.reverse();
    while let Some(n) = stack.pop() {
        out.add(&n);
        if let NodeData::Element { name, .. } = &n.data
            && &*name.local == "template"
        {
            continue;
        }
        let mut kids: Vec<Handle> = children(&n).collect();
        kids.reverse();
        stack.extend(kids);
    }
}

/// The element `node` has the feature.
fn has(kind: Kind, feature: &str, node: &Handle) -> bool {
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

#[cfg(test)]
mod logical_tests;

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
//! composed into paths, a path starts only at the mutated elements that pass
//! the unchanging part of the compound they would match (`Admits`), and the
//! subject feature of the rule (its id, class, tag or attribute) filters the
//! elements a path reaches.

use super::{Changed, Kind, kind, mentions, name};
use crate::style::css::{PseudoKind, Rule, Selector};
use admits::Admits;
use markup5ever_rcdom::Handle;

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

/// The paths of the rules that read a changed name: the unchanging part of
/// the compound a mutated element must pass (`None`: any), the moves from it
/// to the rule's subject, and the subject feature (`None`: any).
pub struct Features(Vec<Path>);

/// The test of a mutated element, the moves to the subject, its feature.
type Path = (Option<Admits>, Vec<Move>, Option<(Kind, String)>);

/// The moves of one reading compound to the subject, and the outermost
/// compound whose element is the mutated element (`None`: not known).
type Reach<'s> = (Vec<Move>, Option<&'s Selector>);

/// The paths of `rules` for the names of `changed`.
pub fn features(rules: &[Rule], changed: &Changed) -> Features {
    let mut out: Vec<Path> = Vec::new();
    for rule in rules {
        let paths = paths(&rule.sel, changed);
        if paths.is_empty() {
            continue;
        }
        let feature = kind(&rule.sel).map(|k| (k, name(&rule.sel, k).to_string()));
        for (path, test) in paths {
            // The mutated elements are always targets: an empty path adds nothing.
            if path.is_empty() {
                continue;
            }
            let entry = match path.contains(&Move::Document) {
                true => (None, vec![Move::Document], feature.clone()),
                false => (test.map(|s| Admits::of(s, changed)), path, feature.clone()),
            };
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
    /// The elements `mutated` and those each path reaches from the ones its
    /// test admits, with the subject feature of its rule, in that order, each once.
    pub fn targets(&self, mutated: &[Handle]) -> Vec<Handle> {
        let mut out = Targets::default();
        for m in mutated {
            out.add(m);
        }
        for (test, path, feature) in &self.0 {
            let admitted = |m: &&Handle| test.as_ref().is_none_or(|t| t.admits(m));
            let mut set: Vec<Handle> = mutated.iter().filter(admitted).cloned().collect();
            let Some((&last, steps)) = path.split_last() else {
                continue;
            };
            for &step in steps {
                set = apply(step, &set);
            }
            reach(last, &set, &mut |node| {
                if feature.as_ref().is_none_or(|(k, f)| has(*k, f, node)) {
                    out.add(node);
                }
            });
        }
        out.list
    }
}

/// The moves from an element whose names changed to the subject of `sel`,
/// one path per compound (or argument) that reads a changed name.
fn paths<'s>(sel: &'s Selector, changed: &Changed) -> Vec<Reach<'s>> {
    let mut out = own(sel, changed);
    let links = [(&sel.ancestor, Move::Children, Move::Descendants)]
        .into_iter()
        .chain([(&sel.prev, Move::NextSibling, Move::LaterSiblings)]);
    for (link, near, far) in links {
        if let Some(link) = link {
            let step = if link.1 { near } else { far };
            out.extend(paths(&link.0, changed).into_iter().map(|(mut p, test)| {
                p.push(step);
                (p, test)
            }));
        }
    }
    out
}

/// The paths that end at the element of the compound `sel` itself: its own
/// classes, attributes and id, and the arguments of its pseudo-classes.
fn own<'s>(sel: &'s Selector, changed: &Changed) -> Vec<Reach<'s>> {
    let among = |names: &[String], n: &str| names.iter().any(|c| c.eq_ignore_ascii_case(n));
    let mut out = Vec::new();
    if sel.classes.iter().any(|c| among(&changed.classes, c))
        || sel.attrs.iter().any(|a| among(&changed.attrs, &a.name))
        || sel.id.is_some() && among(&changed.attrs, "id")
    {
        out.push((Vec::new(), Some(sel)));
    }
    for raw in sel.pseudo.iter().chain(&sel.also) {
        out.extend(pseudo(sel, raw, changed));
    }
    out
}

/// The paths through the pseudo-class `raw` of the compound `sel`.
fn pseudo<'s>(sel: &'s Selector, raw: &str, changed: &Changed) -> Vec<Reach<'s>> {
    let among = |names: &[&str]| {
        names
            .iter()
            .any(|n| changed.attrs.iter().any(|a| a.eq_ignore_ascii_case(n)))
    };
    let text = || {
        (changed.classes.iter())
            .chain(&changed.attrs)
            .any(|n| mentions(raw, n))
    };
    let anywhere = || vec![(vec![Move::Document], None)];
    let Some(arg) = sel.prepared(raw) else {
        // Not parsed here (or a pseudo-element): a name in it may be read anywhere.
        return if text() { anywhere() } else { Vec::new() };
    };
    // A path that has not moved yet is at the element of `sel` too, which
    // must then match all of `sel`: its test is that compound.
    let inner = |step: Option<Move>| -> Vec<Reach<'s>> {
        let reached = arg.list.iter().flat_map(|s| paths(s, changed));
        let lift = |(mut p, test): Reach<'s>| match step {
            Some(step) => {
                p.push(step);
                (p, test)
            }
            None if p.is_empty() => (p, Some(sel)),
            None => (p, test),
        };
        reached.map(lift).collect()
    };
    // A name the argument's text holds where no parsed selector reads it
    // (a part this parser dropped) may be read anywhere.
    let traced = |paths: Vec<Reach<'s>>| {
        if paths.is_empty() && text() {
            anywhere()
        } else {
            paths
        }
    };
    match arg.kind {
        PseudoKind::Not | PseudoKind::Is => traced(inner(None)),
        PseudoKind::NthOf(_) => traced(inner(Some(Move::Siblings))),
        PseudoKind::Has(_) => traced(inner(Some(Move::HasAnchors))),
        // `:checked` reads the attributes its state derives from (`form_state`).
        PseudoKind::Checked => match among(&crate::dom::form_state::READS) {
            true => vec![(Vec::new(), None)],
            false => Vec::new(),
        },
        // The language and direction of an element come from its ancestors.
        PseudoKind::Lang | PseudoKind::Dir(_) => {
            if among(&["lang", "dir", "xml:lang"]) || text() {
                vec![(Vec::new(), None), (vec![Move::Descendants], None)]
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
            match !inner(None).is_empty() || text() {
                true => anywhere(),
                false => Vec::new(),
            }
        }
    }
}

mod admits;
mod walk;
use walk::{Targets, apply, has, reach};

#[cfg(test)]
mod logical_tests;

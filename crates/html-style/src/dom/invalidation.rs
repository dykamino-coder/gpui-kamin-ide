//! Restyle invalidation of a batch of attribute mutations: which elements a
//! rebuild of the same source DOM restyles.
//!
//! Stylo keys the rules of a document by the classes, ids and attributes
//! their selectors read (`InvalidationMap`, `invalidation_map.rs:318`), and
//! Blink keeps an invalidation set per feature (`RuleInvalidationData`,
//! `rule_invalidation_data.h:264`) whose descendant and sibling sets carry
//! the features of the rule's subject (`invalidation_set.h:96`). Both then
//! walk the tree from the mutated element to find the elements to restyle.
//! Here the rebuild matches every element anyway, so the subject features of
//! the dependent rules are checked as the walk leaves each element (`visit`)
//! instead of in a second walk: an element whose matched rules may differ
//! has every simple selector of a rule subject that names no mutated name,
//! and only mutated elements change their names. The map holds only the
//! batch's names, built from the rules of the rebuild (`prepare`), so a
//! document pays nothing for it outside an edit.

use crate::style::css::{Rule, Selector};

/// What a batch of attribute mutations changed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Changed {
    /// Node ids of the mutated elements, each restyled.
    pub elements: Vec<u64>,
    /// Added or removed class tokens, ASCII lowercase.
    pub classes: Vec<String>,
    /// Names of the changed attributes, ASCII lowercase.
    pub attrs: Vec<String>,
}

/// Which simple selector of a rule subject an element the rule matches has
/// (the feature of a Blink invalidation set); the name stays in the rule.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Id,
    Class,
    Attr,
    Tag,
}

/// The feature `Kind` of the rule at an index of the prepared rules.
type Feature = (usize, Kind);

/// Restyled elements found past this many (and mutated elements past this
/// many): the rebuild compares every box.
const LIMIT: usize = 64;

/// Distinct subject features past this many: any element may restyle.
const FEATURES: usize = 8;

/// The invalidation of one rebuild: the batch and the elements it restyles.
/// It lives with the caller for one build and allocates nothing: features
/// point into the rules of the build, ids are kept inline.
#[derive(Debug)]
pub struct Invalidation {
    changed: Changed,
    /// Subject features of the rules that read a changed name, and the rules
    /// they index (address and length); `None`: a dependent rule names no
    /// feature of its subject (or styles a shadow tree), or too many, so any
    /// element may restyle.
    features: Option<([Feature; FEATURES], usize, (usize, usize))>,
    /// The mutated elements, then the elements the walk found restyled.
    ids: [u64; 2 * LIMIT],
    len: usize,
}

impl Invalidation {
    /// The batch `changed`, before its rules are known.
    pub fn new(changed: Changed) -> Self {
        Self {
            changed,
            features: None,
            ids: [0; 2 * LIMIT],
            len: 0,
        }
    }

    /// The rules of the rebuild are `rules` (`shadows`: some apply in shadow
    /// trees, whose hosts and slots this map does not follow).
    pub fn prepare(&mut self, rules: &[Rule], shadows: bool) {
        self.features = None;
        let elements = &self.changed.elements;
        if shadows || elements.len() > LIMIT {
            return;
        }
        self.ids[..elements.len()].copy_from_slice(elements);
        self.len = elements.len();
        let mut features = [(0, Kind::Tag); FEATURES];
        let mut count = 0;
        for (at, rule) in rules.iter().enumerate() {
            if !reads(&rule.sel, &self.changed) {
                continue;
            }
            let Some(kind) = kind(&rule.sel) else {
                return;
            };
            let name = name(&rule.sel, kind);
            let known = features[..count].iter().any(|&(i, k)| {
                k == kind && name.eq_ignore_ascii_case(self::name(&rules[i].sel, k))
            });
            if known {
                continue;
            }
            if count == FEATURES {
                return;
            }
            features[count] = (at, kind);
            count += 1;
        }
        self.features = Some((features, count, (rules.as_ptr() as usize, rules.len())));
    }

    /// The walk left the element `id` of tag `tag` with attributes `attrs`;
    /// `rules`: the rules of its scope.
    pub fn visit(&mut self, rules: &[Rule], tag: &str, attrs: &[(String, String)], id: u64) {
        let Some((features, count, prepared)) = &self.features else {
            return;
        };
        // Another scope's rules (a shadow tree is not followed anyway).
        if *prepared != (rules.as_ptr() as usize, rules.len()) {
            self.features = None;
            return;
        }
        let hit = features[..*count]
            .iter()
            .any(|&(at, kind)| has(kind, name(&rules[at].sel, kind), tag, attrs));
        if hit {
            if self.len == self.changed.elements.len() + LIMIT {
                self.features = None;
                return;
            }
            self.ids[self.len] = id;
            self.len += 1;
        }
    }

    /// Node ids of the elements restyled by the batch; `None`: any may be.
    pub fn finish(&self) -> Option<&[u64]> {
        self.features.as_ref()?;
        Some(&self.ids[..self.len])
    }
}

/// `sel` reads a name of `changed` in one of its compounds or arguments.
fn reads(sel: &Selector, changed: &Changed) -> bool {
    let among = |names: &[String], name: &str| names.iter().any(|n| n.eq_ignore_ascii_case(name));
    // An unparsed argument is read as text: any mention of a name counts.
    let text = |raw: &String| {
        let mut names = changed.classes.iter().chain(&changed.attrs);
        raw.contains('\\') || names.any(|n| mentions(raw, n))
    };
    sel.classes.iter().any(|c| among(&changed.classes, c))
        || sel.attrs.iter().any(|a| among(&changed.attrs, &a.name))
        || sel.pseudo.iter().chain(&sel.also).any(text)
        || sel
            .arguments
            .iter()
            .any(|p| p.list.iter().any(|s| reads(s, changed)))
        || sel.ancestor.as_ref().is_some_and(|a| reads(&a.0, changed))
        || sel.prev.as_ref().is_some_and(|p| reads(&p.0, changed))
}

/// `raw` has `name` (ASCII lowercase) somewhere, ignoring ASCII case.
fn mentions(raw: &str, name: &str) -> bool {
    let (raw, name) = (raw.as_bytes(), name.as_bytes());
    !name.is_empty()
        && raw
            .windows(name.len())
            .any(|w| w.eq_ignore_ascii_case(name))
}
/// The most selective feature of the subject of `sel`: an id, a class, a
/// tag, then an attribute name, which many elements carry; `None`: none.
fn kind(sel: &Selector) -> Option<Kind> {
    if sel.id.is_some() {
        Some(Kind::Id)
    } else if !sel.classes.is_empty() {
        Some(Kind::Class)
    } else if sel.tag.as_ref().is_some_and(|t| t != "*") {
        Some(Kind::Tag)
    } else if !sel.attrs.is_empty() {
        Some(Kind::Attr)
    } else {
        None
    }
}

/// The name of the feature `kind` of `sel` (`kind` gave it).
fn name(sel: &Selector, kind: Kind) -> &str {
    let name = match kind {
        Kind::Id => sel.id.as_deref(),
        Kind::Class => sel.classes.first().map(String::as_str),
        Kind::Tag => sel.tag.as_deref(),
        Kind::Attr => sel.attrs.first().map(|a| a.name.as_str()),
    };
    name.unwrap_or_default()
}

/// The element of `tag` and `attrs` has the feature `kind` named `name`,
/// ignoring ASCII case.
fn has(kind: Kind, name: &str, tag: &str, attrs: &[(String, String)]) -> bool {
    let value = |n: &str| {
        attrs
            .iter()
            .find(|(a, _)| a.eq_ignore_ascii_case(n))
            .map(|(_, v)| v.as_str())
    };
    match kind {
        Kind::Tag => tag.eq_ignore_ascii_case(name),
        Kind::Attr => value(name).is_some(),
        Kind::Id => value("id").is_some_and(|v| v.eq_ignore_ascii_case(name)),
        Kind::Class => value("class").is_some_and(|v| {
            v.split_ascii_whitespace()
                .any(|t| t.eq_ignore_ascii_case(name))
        }),
    }
}

#[cfg(test)]
mod tests;

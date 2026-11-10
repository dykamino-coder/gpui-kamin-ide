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

/// A simple selector of a rule subject: an element the rule matches has it
/// (the feature of a Blink invalidation set).
#[derive(Debug, PartialEq)]
enum Feature {
    Id(String),
    Class(String),
    Attr(String),
    Tag(String),
}

/// Restyled elements found past this many: the rebuild compares every box.
const LIMIT: usize = 64;

/// The invalidation of one rebuild: the batch and the elements it restyles.
#[derive(Debug)]
pub struct Invalidation {
    changed: Changed,
    /// Subject features of the rules that read a changed name; `None`: a
    /// dependent rule names no feature of its subject (or styles a shadow
    /// tree), so any element may restyle.
    features: Option<Vec<Feature>>,
    restyled: Vec<u64>,
}

impl Invalidation {
    /// The batch `changed`, before its rules are known.
    pub fn new(changed: Changed) -> Self {
        Self {
            changed,
            features: None,
            restyled: Vec::new(),
        }
    }

    /// The rules of the rebuild are `rules` (`shadows`: some apply in shadow
    /// trees, whose hosts and slots this map does not follow).
    pub fn prepare(&mut self, rules: &[Rule], shadows: bool) {
        self.restyled.clear();
        let mut features = Vec::new();
        for rule in rules {
            if !reads(&rule.sel, &self.changed) {
                continue;
            }
            match feature(&rule.sel) {
                Some(f) if !features.contains(&f) => features.push(f),
                Some(_) => {}
                None => {
                    self.features = None;
                    return;
                }
            }
        }
        self.features = (!shadows).then_some(features);
    }

    /// The walk left the element `id` of tag `tag` with attributes `attrs`.
    pub fn visit(&mut self, tag: &str, attrs: &[(String, String)], id: u64) {
        let Some(features) = &self.features else {
            return;
        };
        if features.iter().any(|f| has(f, tag, attrs)) {
            if self.restyled.len() == LIMIT {
                self.features = None;
                return;
            }
            self.restyled.push(id);
        }
    }

    /// Node ids of the elements restyled by the batch; `None`: any may be.
    pub fn finish(self) -> Option<Vec<u64>> {
        self.features?;
        let mut ids = self.changed.elements;
        ids.extend(self.restyled);
        Some(ids)
    }
}

/// `sel` reads a name of `changed` in one of its compounds or arguments.
fn reads(sel: &Selector, changed: &Changed) -> bool {
    let named = |name: &str| {
        let name = name.to_ascii_lowercase();
        changed.classes.contains(&name)
    };
    let attr = |name: &str| changed.attrs.contains(&name.to_ascii_lowercase());
    // An unparsed argument is read as text: any mention of a name counts.
    let text = |raw: &String| {
        let raw = raw.to_ascii_lowercase();
        changed
            .classes
            .iter()
            .chain(&changed.attrs)
            .any(|n| raw.contains(n.as_str()))
            || raw.contains('\\')
    };
    sel.classes.iter().any(|c| named(c))
        || sel.attrs.iter().any(|a| attr(&a.name))
        || sel.pseudo.iter().chain(&sel.also).any(text)
        || sel
            .arguments
            .iter()
            .any(|p| p.list.iter().any(|s| reads(s, changed)))
        || sel.ancestor.as_ref().is_some_and(|a| reads(&a.0, changed))
        || sel.prev.as_ref().is_some_and(|p| reads(&p.0, changed))
}

/// The most selective feature of the subject of `sel`: an id, a class, a
/// tag, then an attribute name, which many elements carry; `None`: none.
fn feature(sel: &Selector) -> Option<Feature> {
    let low = |s: &String| s.to_ascii_lowercase();
    if let Some(id) = &sel.id {
        return Some(Feature::Id(low(id)));
    }
    if let Some(class) = sel.classes.first() {
        return Some(Feature::Class(low(class)));
    }
    if let Some(tag) = sel.tag.as_ref().filter(|t| *t != "*") {
        return Some(Feature::Tag(low(tag)));
    }
    sel.attrs.first().map(|a| Feature::Attr(low(&a.name)))
}

/// The element of `tag` and `attrs` has `feature`, ignoring ASCII case.
fn has(feature: &Feature, tag: &str, attrs: &[(String, String)]) -> bool {
    let value = |name: &str| {
        attrs
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    };
    match feature {
        Feature::Tag(t) => tag.eq_ignore_ascii_case(t),
        Feature::Attr(a) => value(a).is_some(),
        Feature::Id(id) => value("id").is_some_and(|v| v.eq_ignore_ascii_case(id)),
        Feature::Class(c) => value("class").is_some_and(|v| {
            v.split_ascii_whitespace()
                .any(|t| t.eq_ignore_ascii_case(c))
        }),
    }
}

#[cfg(test)]
mod tests;

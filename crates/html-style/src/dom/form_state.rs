//! Checkedness of form controls, the state `:checked` matches (selectors-4
//! В§14.3.1, HTML В§4.16.3): `input` checkboxes and radio buttons, and the
//! selectedness of `option` elements.
//!
//! The state is derived from the content attributes as a document without
//! user input or script has it (HTML В§4.10.5.1.15 "checkedness", В§4.10.10
//! "selectedness setting algorithm"): a radio group keeps only its last
//! checked button in tree order, a single-select keeps its last selected
//! option, or (for a one-row select) its first enabled one. Blink keeps the same state on the
//! element (`HTMLInputElement::checked_`, `HTMLOptionElement::is_selected_`)
//! and invalidates `:checked` when it changes; here it is a side table of
//! the parse (`TreeIndex::form`), so a later input or script write is one
//! record of the nodes whose state changed, restyled through
//! `dom::invalidation` like an attribute mutation of `checked`.

use crate::dom::tree_index::HandleKey;
use markup5ever_rcdom::{Handle, NodeData};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

mod selection;

const HTML: &str = "http://www.w3.org/1999/xhtml";

/// Attributes whose mutation may change some element's checkedness.
pub const READS: [&str; 9] = [
    "checked", "selected", "type", "name", "multiple", "size", "disabled", "form", "id",
];

/// Radio buttons that win their group, computed once per tree root on the
/// first checked radio button that asks.
#[derive(Default)]
pub struct FormState {
    radios: RefCell<HashMap<HandleKey, HashSet<HandleKey>>>,
    selections: RefCell<HashMap<HandleKey, Option<HandleKey>>>,
}

impl FormState {
    /// `:checked` of the element `handle`, an HTML `tag` with `attrs`.
    pub fn checked(&self, handle: &Handle, tag: &str, attrs: &[(String, String)]) -> bool {
        let has = |name: &str| attrs.iter().any(|(k, _)| k == name);
        match tag {
            "input" if html(handle) => match kind(attrs) {
                Some(Choice::Checkbox) => has("checked"),
                Some(Choice::Radio) => has("checked") && self.wins_group(handle, attrs),
                None => false,
            },
            "option" if html(handle) => self.selected(handle, attrs),
            _ => false,
        }
    }

    fn wins_group(&self, handle: &Handle, attrs: &[(String, String)]) -> bool {
        if value(attrs, "name").is_none_or(str::is_empty) {
            return true;
        }
        let root = HandleKey::of(&root_of(handle));
        let mut radios = self.radios.borrow_mut();
        let winners = radios
            .entry(root.clone())
            .or_insert_with(|| group_winners(root.handle()));
        winners.contains(&HandleKey::of(handle))
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Choice {
    Checkbox,
    Radio,
}

fn kind(attrs: &[(String, String)]) -> Option<Choice> {
    let ty = value(attrs, "type")?;
    if ty.eq_ignore_ascii_case("checkbox") {
        Some(Choice::Checkbox)
    } else if ty.eq_ignore_ascii_case("radio") {
        Some(Choice::Radio)
    } else {
        None
    }
}

fn value<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

fn html(handle: &Handle) -> bool {
    matches!(&handle.data, NodeData::Element { name, .. } if &*name.ns == HTML)
}

fn parent(handle: &Handle) -> Option<Handle> {
    let weak = handle.parent.take();
    let up = weak.as_ref().and_then(|p| p.upgrade());
    handle.parent.set(weak);
    up
}

fn root_of(handle: &Handle) -> Handle {
    let mut at = handle.clone();
    while let Some(up) = parent(&at) {
        at = up;
    }
    at
}

fn element(handle: &Handle) -> Option<(&str, Vec<(String, String)>)> {
    let NodeData::Element { name, attrs, .. } = &handle.data else {
        return None;
    };
    let attrs = attrs.borrow();
    let list = attrs
        .iter()
        .map(|a| (a.name.local.to_string(), a.value.to_string()))
        .collect();
    Some((&*name.local, list))
}

/// Group key of a radio button: its form owner (the `form` attribute, else
/// the nearest `form` ancestor) and its name (HTML В§4.10.5.1.15).
fn group_key(
    handle: &Handle,
    attrs: &[(String, String)],
    forms: &HashMap<String, Option<usize>>,
) -> (Option<usize>, String) {
    let name = value(attrs, "name").unwrap_or_default().to_string();
    if let Some(form) = value(attrs, "form") {
        return (forms.get(form).copied().flatten(), name);
    }
    let mut at = parent(handle);
    while let Some(node) = at {
        if html(&node) && element(&node).is_some_and(|(tag, _)| tag == "form") {
            let id = std::rc::Rc::as_ptr(&node) as usize;
            return (Some(id), name);
        }
        at = parent(&node);
    }
    (None, name)
}

/// The last checked radio button of each named group under `root`, in tree
/// order: checking one unchecks the rest of its group while parsing.
fn group_winners(root: &Handle) -> HashSet<HandleKey> {
    let mut forms = HashMap::new();
    let mut candidates = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(node) = stack.pop() {
        if let Some((tag, attrs)) = element(&node) {
            // Explicit associations use the first element with this ID, even
            // if it is not a form; resolve after visiting forward references.
            if let Some(id) = value(&attrs, "id").filter(|id| !id.is_empty()) {
                forms.entry(id.to_string()).or_insert_with(|| {
                    (html(&node) && tag == "form").then(|| std::rc::Rc::as_ptr(&node) as usize)
                });
            }
            if html(&node)
                && tag == "input"
                && kind(&attrs) == Some(Choice::Radio)
                && value(&attrs, "checked").is_some()
                && value(&attrs, "name").is_some_and(|n| !n.is_empty())
            {
                candidates.push((node.clone(), attrs));
            }
        }
        stack.extend(node.children.borrow().iter().rev().cloned());
    }
    let mut last = HashMap::new();
    for (node, attrs) in candidates {
        last.insert(group_key(&node, &attrs, &forms), node);
    }
    last.values().map(HandleKey::of).collect()
}

#[cfg(test)]
mod tests;

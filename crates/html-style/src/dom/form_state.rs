//! Checkedness of form controls, the state `:checked` matches (selectors-4
//! §14.3.1, HTML §4.16.3): `input` checkboxes and radio buttons, and the
//! selectedness of `option` elements.
//!
//! The state is derived from the content attributes as a document without
//! user input or script has it (HTML §4.10.5.1.15 "checkedness", §4.10.10
//! "selectedness setting algorithm"): a radio group keeps only its last
//! checked button in tree order, a single-select keeps its last selected
//! option, or else its first enabled one. Blink keeps the same state on the
//! element (`HTMLInputElement::checked_`, `HTMLOptionElement::is_selected_`)
//! and invalidates `:checked` when it changes; here it is a side table of
//! the parse (`TreeIndex::form`), so a later input or script write is one
//! record of the nodes whose state changed, restyled through
//! `dom::invalidation` like an attribute mutation of `checked`.

use crate::dom::tree_index::HandleKey;
use markup5ever_rcdom::{Handle, NodeData};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

const HTML: &str = "http://www.w3.org/1999/xhtml";

/// Attributes whose mutation may change some element's checkedness.
pub const READS: [&str; 7] = [
    "checked", "selected", "type", "name", "multiple", "size", "disabled",
];

/// Radio buttons that win their group, computed once per tree root on the
/// first checked radio button that asks.
#[derive(Default)]
pub struct FormState {
    radios: RefCell<HashMap<HandleKey, HashSet<HandleKey>>>,
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
            "option" if html(handle) => selected(handle, attrs),
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
/// the nearest `form` ancestor) and its name (HTML §4.10.5.1.15).
fn group_key(handle: &Handle, attrs: &[(String, String)]) -> (Option<String>, String) {
    let name = value(attrs, "name").unwrap_or_default().to_string();
    if let Some(form) = value(attrs, "form") {
        return (Some(format!("#{form}")), name);
    }
    let mut at = parent(handle);
    while let Some(node) = at {
        if html(&node) && element(&node).is_some_and(|(tag, _)| tag == "form") {
            let id = std::rc::Rc::as_ptr(&node) as usize;
            return (Some(format!("@{id:x}")), name);
        }
        at = parent(&node);
    }
    (None, name)
}

/// The last checked radio button of each named group under `root`, in tree
/// order: checking one unchecks the rest of its group while parsing.
fn group_winners(root: &Handle) -> HashSet<HandleKey> {
    let mut last: HashMap<(Option<String>, String), Handle> = HashMap::new();
    let mut stack = vec![root.clone()];
    while let Some(node) = stack.pop() {
        if html(&node)
            && let Some((tag, attrs)) = element(&node)
            && tag == "input"
            && kind(&attrs) == Some(Choice::Radio)
            && value(&attrs, "checked").is_some()
            && value(&attrs, "name").is_some_and(|n| !n.is_empty())
        {
            last.insert(group_key(&node, &attrs), node.clone());
        }
        stack.extend(node.children.borrow().iter().rev().cloned());
    }
    last.values().map(HandleKey::of).collect()
}

/// Selectedness of an `option` (HTML §4.10.10 "selectedness setting
/// algorithm"); outside a `select` it is the `selected` attribute.
fn selected(option: &Handle, attrs: &[(String, String)]) -> bool {
    let own = value(attrs, "selected").is_some();
    let Some(select) = owner_select(option) else {
        return own;
    };
    let Some((_, select_attrs)) = element(&select) else {
        return own;
    };
    let multiple = value(&select_attrs, "multiple").is_some();
    let size = value(&select_attrs, "size")
        .and_then(|s| s.trim().parse::<u32>().ok())
        .filter(|&s| s > 0)
        .unwrap_or(if multiple { 4 } else { 1 });
    if multiple || size > 1 {
        return own;
    }
    let options = list_of_options(&select);
    let chosen = options
        .iter()
        .rev()
        .find(|(_, attrs, _)| value(attrs, "selected").is_some())
        .or_else(|| options.iter().find(|(_, _, disabled)| !disabled));
    chosen.is_some_and(|(h, _, _)| std::rc::Rc::ptr_eq(h, option))
}

/// The `select` whose list of options holds `option`: its parent, or the
/// parent of its `optgroup` parent.
fn owner_select(option: &Handle) -> Option<Handle> {
    let up = parent(option)?;
    let (tag, _) = element(&up)?;
    match tag {
        "select" => Some(up),
        "optgroup" => parent(&up).filter(|s| element(s).is_some_and(|(t, _)| t == "select")),
        _ => None,
    }
}

type Listed = (Handle, Vec<(String, String)>, bool);

/// The list of options of `select` in tree order, each with its attributes
/// and whether it is disabled (its own attribute or its `optgroup`'s).
fn list_of_options(select: &Handle) -> Vec<Listed> {
    let mut out = Vec::new();
    let mut push = |node: &Handle, group_disabled: bool| {
        if let Some(("option", attrs)) = element(node) {
            let disabled = group_disabled || value(&attrs, "disabled").is_some();
            out.push((node.clone(), attrs, disabled));
        }
    };
    for child in select.children.borrow().iter() {
        match element(child) {
            Some(("optgroup", attrs)) => {
                let disabled = value(&attrs, "disabled").is_some();
                for grand in child.children.borrow().iter() {
                    push(grand, disabled);
                }
            }
            Some(_) => push(child, false),
            None => {}
        }
    }
    out
}

#[cfg(test)]
mod tests;

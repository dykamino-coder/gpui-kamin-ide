//! Static select reset: option tree order, explicit selectedness and fallback.

use crate::{Handle, Node, NodeData};
use std::rc::Rc;

fn html_name(node: &Node) -> Option<&str> {
    match &node.data {
        NodeData::Element { name, .. } if &*name.ns == "http://www.w3.org/1999/xhtml" => {
            Some(name.local.as_ref())
        }
        _ => None,
    }
}

fn attr(node: &Node, key: &str) -> Option<String> {
    let NodeData::Element { attrs, .. } = &node.data else {
        return None;
    };
    attrs.borrow().iter().find_map(|a| {
        (a.name.ns.is_empty() && &*a.name.local == key).then(|| a.value.to_string())
    })
}

// HTML's non-negative integer parser consumes a digit prefix, accepts '+'
// and ASCII whitespace, and defaults an invalid/missing size to one here.
fn display_size_is_one(select: &Node) -> bool {
    let Some(size) = attr(select, "size") else {
        return true;
    };
    let size = size.trim_start_matches(['\t', '\n', '\x0c', '\r', ' ']);
    let size = size.strip_prefix('+').unwrap_or(size);
    let digits: String = size.chars().take_while(char::is_ascii_digit).collect();
    digits.is_empty() || digits.trim_start_matches('0') == "1"
}

impl Node {
    // KaminIDE patch: HTML select's list of options is depth-first tree order.
    // See https://html.spec.whatwg.org/multipage/form-elements.html#concept-select-option-list
    fn reset_options(self: &Rc<Self>) -> Vec<(Handle, bool)> {
        let mut options = Vec::new();
        let mut stack: Vec<_> = self
            .children
            .borrow()
            .iter()
            .rev()
            .cloned()
            .map(|node| (node, false, false))
            .collect();
        while let Some((node, in_group, disabled_group)) = stack.pop() {
            let name = html_name(&node);
            if name == Some("option") {
                let disabled = disabled_group || attr(&node, "disabled").is_some();
                options.push((node, disabled));
                continue;
            }
            if matches!(name, Some("select" | "hr" | "datalist"))
                || (name == Some("optgroup") && in_group)
            {
                continue;
            }
            let group = name == Some("optgroup");
            let disabled = disabled_group || (group && attr(&node, "disabled").is_some());
            stack.extend(
                node.children
                    .borrow()
                    .iter()
                    .rev()
                    .cloned()
                    .map(|child| (child, in_group || group, disabled)),
            );
        }
        options
    }

    /// KaminIDE patch: resolve the static parse tree's reset selectedness.
    /// https://html.spec.whatwg.org/multipage/form-elements.html#selectedness-setting-algorithm
    pub(crate) fn selected_option(self: &Rc<Self>) -> Option<Handle> {
        let options = self.reset_options();
        let mut selected = options
            .iter()
            .filter(|(node, _)| attr(node, "selected").is_some());
        if attr(self, "multiple").is_some() {
            return selected.map(|(node, _)| node.clone()).next();
        }
        selected
            .next_back()
            .map(|(node, _)| node.clone())
            .or_else(|| {
                display_size_is_one(self)
                    .then(|| {
                        options
                            .iter()
                            .find(|(_, disabled)| !disabled)
                            .map(|(node, _)| node.clone())
                    })
                    .flatten()
            })
    }

    pub(crate) fn is_selected_option(self: &Rc<Self>) -> bool {
        let Some(select) = self.get_option_element_nearest_ancestor_select() else {
            return false;
        };
        if attr(&select, "multiple").is_some() {
            return attr(self, "selected").is_some();
        }
        select
            .selected_option()
            .is_some_and(|chosen| Rc::ptr_eq(&chosen, self))
    }
}

#[cfg(test)]
#[path = "selection_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "selectedcontent_tests.rs"]
mod selectedcontent_tests;

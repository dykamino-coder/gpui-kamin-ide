//! Static select reset shares admission with the canonical DOM mutations.

use crate::admission::{Admission, Work};
use crate::{Handle, Node, NodeData};
use std::rc::Rc;
use tendril::StrTendril;

fn html_name(node: &Node) -> Option<&str> {
    match &node.data {
        NodeData::Element { name, .. } if &*name.ns == "http://www.w3.org/1999/xhtml" => {
            Some(name.local.as_ref())
        }
        _ => None,
    }
}

fn attr<A: Admission>(node: &Node, key: &str, policy: &A) -> Result<Option<StrTendril>, A::Error> {
    if let NodeData::Element { attrs, .. } = &node.data {
        for attr in attrs.borrow().iter() {
            policy.visit()?;
            if attr.name.ns.is_empty() && &*attr.name.local == key {
                return Ok(Some(attr.value.clone()));
            }
        }
    }
    Ok(None)
}

// HTML's non-negative integer parser consumes a digit prefix and accepts '+'.
fn display_size_is_one<A: Admission>(select: &Node, policy: &A) -> Result<bool, A::Error> {
    let Some(size) = attr(select, "size", policy)? else {
        return Ok(true);
    };
    let size = size.trim_start_matches(['\t', '\n', '\x0c', '\r', ' ']);
    let size = size.strip_prefix('+').unwrap_or(size);
    let end = size
        .bytes()
        .position(|byte| !byte.is_ascii_digit())
        .unwrap_or(size.len());
    let digits = &size[..end];
    Ok(digits.is_empty() || digits.trim_start_matches('0') == "1")
}

impl Node {
    // https://html.spec.whatwg.org/multipage/form-elements.html#concept-select-option-list
    fn reset_options<A: Admission>(
        self: &Rc<Self>,
        policy: &A,
    ) -> Result<Work<(Handle, bool)>, A::Error> {
        let mut options = Work::new();
        let mut stack = Work::new();
        for node in self.children.borrow().iter().rev() {
            stack.push((node.clone(), false, false), policy)?;
        }
        while let Some((node, in_group, disabled_group)) = stack.pop() {
            policy.visit()?;
            let name = html_name(&node);
            if name == Some("option") {
                let disabled = disabled_group || attr(&node, "disabled", policy)?.is_some();
                options.push((node, disabled), policy)?;
                continue;
            }
            if matches!(name, Some("select" | "hr" | "datalist"))
                || (name == Some("optgroup") && in_group)
            {
                continue;
            }
            let group = name == Some("optgroup");
            let disabled = disabled_group || (group && attr(&node, "disabled", policy)?.is_some());
            for child in node.children.borrow().iter().rev() {
                stack.push((child.clone(), in_group || group, disabled), policy)?;
            }
        }
        Ok(options)
    }

    // https://html.spec.whatwg.org/multipage/form-elements.html#selectedness-setting-algorithm
    pub(crate) fn try_selected_option<A: Admission>(
        self: &Rc<Self>,
        policy: &A,
    ) -> Result<Option<Handle>, A::Error> {
        let options = self.reset_options(policy)?;
        let multiple = attr(self, "multiple", policy)?.is_some();
        let mut selected = None;
        for (node, _) in &options.values {
            if attr(node, "selected", policy)?.is_some() {
                selected = Some(node.clone());
                if multiple {
                    return Ok(selected);
                }
            }
        }
        if selected.is_some() || multiple {
            return Ok(selected);
        }
        if display_size_is_one(self, policy)? {
            return Ok(options
                .values
                .iter()
                .find(|(_, disabled)| !disabled)
                .map(|(node, _)| node.clone()));
        }
        Ok(None)
    }

    pub(crate) fn try_is_selected_option<A: Admission>(
        self: &Rc<Self>,
        policy: &A,
    ) -> Result<bool, A::Error> {
        let Some(select) = self.try_nearest_select(policy)? else {
            return Ok(false);
        };
        if attr(&select, "multiple", policy)?.is_some() {
            return Ok(attr(self, "selected", policy)?.is_some());
        }
        Ok(select
            .try_selected_option(policy)?
            .is_some_and(|chosen| Rc::ptr_eq(&chosen, self)))
    }

    #[cfg(test)]
    pub(crate) fn is_selected_option(self: &Rc<Self>) -> bool {
        crate::admission::infallible(self.try_is_selected_option(&crate::admission::Unbounded))
    }

    #[cfg(test)]
    pub(crate) fn get_a_selects_enabled_selectedcontent(&self) -> Option<Handle> {
        crate::admission::infallible(self.try_selectedcontent(&crate::admission::Unbounded))
    }

    #[cfg(test)]
    pub(crate) fn selected_option(self: &Rc<Self>) -> Option<Handle> {
        crate::admission::infallible(self.try_selected_option(&crate::admission::Unbounded))
    }
}

#[cfg(test)]
#[path = "selectedcontent_tests.rs"]
mod selectedcontent_tests;
#[cfg(test)]
#[path = "selection_tests.rs"]
mod tests;

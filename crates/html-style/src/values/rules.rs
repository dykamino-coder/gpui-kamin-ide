//! At-rule accessors of a document's values: `@position-try`, `@property`
//! and `@page` rules, read by the cascade and the host.

use super::{DocumentValues, property_registry};
use crate::style::css::PageRule;
use std::rc::Rc;

impl DocumentValues {
    /// The declarations of the `@position-try` rule named `name` (`--x`), copied.
    pub fn try_rule(&self, name: &str) -> Option<crate::style::css::DeclBlock> {
        self.try_rules.borrow().get(name).cloned()
    }

    /// The immutable revision of the document's `@property` registry, shared by its nodes.
    pub fn property_rules(&self) -> Rc<property_registry::PropertySnapshot> {
        self.property_rules.borrow().snapshot()
    }

    /// The document's `@page` rules, copied (the host reads them every frame).
    pub fn page_rules(&self) -> Vec<PageRule> {
        self.page_rules.borrow().clone()
    }
}

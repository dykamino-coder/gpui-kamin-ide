//! Render implementation of the layout child-build contract.

use crate::layout::build_contracts::ChildBuild;

impl Default for ChildBuild {
    fn default() -> Self {
        ChildBuild::new(super::blocks, super::element, super::paragraph)
    }
}

//! Tests for dom; split out to keep the owning module within 250 lines.

mod style_parsing;
mod tree_parsing;
use crate::dom::tests::tree_parsing::child_colors;
use crate::dom::tests::tree_parsing::first_element;

use super::*;

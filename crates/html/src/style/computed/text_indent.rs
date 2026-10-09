//! Resolve text indentation while preserving an explicit inheritance request.

use super::{Computed, split_outside_parens};
use crate::style::values::value::Len;

pub(super) fn apply(style: &mut Computed, value: &str) {
    // CSS Cascade §7.3.2: inherit overrides an earlier specified value.
    // Unset fields are resolved from the parent by inline::inherit.
    if value.eq_ignore_ascii_case("inherit") {
        style.text_indent = None;
        style.text_indent_each_line = None;
        style.text_indent_hanging = None;
        return;
    }
    let (mut each, mut hang) = (false, false);
    // Keep a calc() expression together: its percentage basis is resolved
    // only when the paragraph receives its containing block width.
    for word in split_outside_parens(value) {
        match word.to_ascii_lowercase().as_str() {
            "each-line" => each = true,
            "hanging" => hang = true,
            len => style.text_indent = Len::parse_mixed(len).or(style.text_indent),
        }
    }
    style.text_indent_each_line = each.then_some(true);
    style.text_indent_hanging = hang.then_some(true);
}

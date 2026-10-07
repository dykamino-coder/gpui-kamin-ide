//! Content language lookup shared by selectors and automatic quotation marks.

use super::Ancestor;

fn own(node: &Ancestor) -> Option<&str> {
    node.attrs
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("lang") || k.eq_ignore_ascii_case("xml:lang"))
        .map(|(_, value)| value.as_str())
}

pub(super) fn effective<'a>(node: &'a Ancestor, path: &'a [Ancestor]) -> Option<&'a str> {
    own(node).or_else(|| path.iter().rev().find_map(own))
}

/// CSS Content 3 §quotes-property: root auto uses its own language.
pub(super) fn parent<'a>(node: &'a Ancestor, path: &'a [Ancestor]) -> Option<&'a str> {
    if let Some((parent, ancestors)) = path.split_last() {
        effective(parent, ancestors)
    } else {
        own(node)
    }
}

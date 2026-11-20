//! The tokens of a `class` attribute, one reading for matching, indexing and
//! invalidation.
//!
//! HTML §2.3.8: a set of space-separated tokens is split on ASCII whitespace
//! (U+0009, U+000A, U+000C, U+000D, U+0020); other spaces such as U+00A0
//! belong to a token. Selectors compare the tokens as written outside quirks
//! mode (Selectors 4 §6.6), so a change of case changes the class (Blink
//! `SpaceSplitString`, Stylo `each_class`).

/// The class tokens of `value`.
pub fn tokens(value: &str) -> impl Iterator<Item = &str> {
    value.split_ascii_whitespace()
}

/// The tokens of one value missing from the other, as written: the classes a
/// change from `was` to `is` adds or removes.
pub fn changed(was: Option<&str>, is: Option<&str>) -> Vec<String> {
    let (a, b): (Vec<&str>, Vec<&str>) = (
        tokens(was.unwrap_or("")).collect(),
        tokens(is.unwrap_or("")).collect(),
    );
    (a.iter().filter(|t| !b.contains(t)))
        .chain(b.iter().filter(|t| !a.contains(t)))
        .map(|t| t.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_split_on_ascii_whitespace_and_keep_their_case() {
        let all: Vec<&str> = tokens(" a\tB\u{a0}c\n\u{c}d\r").collect();
        assert_eq!(all, ["a", "B\u{a0}c", "d"]);
        assert_eq!(changed(Some("A x"), Some("a x")), ["A", "a"]);
        assert!(changed(Some("a  b"), Some("b a")).is_empty());
    }
}

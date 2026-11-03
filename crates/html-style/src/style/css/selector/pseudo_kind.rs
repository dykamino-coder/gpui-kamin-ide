//! Kind of a pseudo-class, decided once when the selector is parsed (Blink
//! `CSSSelector::PseudoType`, Stylo `NonTSPseudoClass`). Matching dispatches on
//! the kind instead of re-reading the pseudo-class text for every element.
//! The text stays on the selector for the layers that apply states later.

/// Position test of a structural pseudo-class (selectors-4 §14.3–§14.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Position {
    First,
    Last,
    Only,
    Nth,
    NthLast,
}

/// What the base cascade evaluates for one pseudo-class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PseudoKind {
    /// `:not(S)`.
    Not,
    /// `:is()`, `:where()` and the legacy `:matches()`/`:any()`.
    Is,
    /// `:has(R)` with the identity of its argument (`has_argument_id`).
    Has(u64),
    /// `:has-slotted` and `:has-slotted(S)`.
    HasSlotted,
    /// `:host` and `:host(S)`.
    Host,
    Link,
    Visited,
    /// `:dir(rtl)` (`true`) or any other direction (`false`).
    Dir(bool),
    TargetCurrent,
    Root,
    Empty,
    /// `:lang(…)`; the ranges are read from the text.
    Lang,
    /// `:nth-child(An+B of S)` (`false`) or `:nth-last-child(An+B of S)` (`true`).
    NthOf(bool),
    /// `:first-child` … `:nth-last-of-type()`; `true` counts siblings of the same type.
    Structural(Position, bool),
    /// `:hover` and `:active`: never held by a non-subject compound in a still frame.
    UserAction,
    /// Applied by another layer or not supported: the base cascade does not match it.
    Other,
}

impl PseudoKind {
    /// Kind of the lower-cased pseudo-class text `raw`; `nth_of` tells whether its
    /// prepared argument is an `of S` form.
    pub fn of(raw: &str, nth_of: bool) -> Self {
        let functional = |name: &str| {
            raw.strip_prefix(name)
                .and_then(|r| r.strip_prefix('('))
                .and_then(|r| r.strip_suffix(')'))
        };
        if raw == "host" || raw.starts_with("host(") {
            return Self::Host;
        }
        if raw.starts_with("has-slotted") {
            return Self::HasSlotted;
        }
        if functional("not").is_some() {
            return Self::Not;
        }
        if ["is", "where", "matches", "any"]
            .iter()
            .any(|name| functional(name).is_some())
        {
            return Self::Is;
        }
        if let Some(arg) = functional("dir") {
            return Self::Dir(arg.trim().eq_ignore_ascii_case("rtl"));
        }
        if functional("lang").is_some() {
            return Self::Lang;
        }
        if let Some(arg) = functional("has") {
            return Self::Has(has_argument_id(arg));
        }
        let name = raw.split_once('(').map_or(raw, |(name, _)| name);
        match name {
            "nth-child" | "nth-last-child" if nth_of && raw.contains('(') => {
                return Self::NthOf(name == "nth-last-child");
            }
            _ => {}
        }
        let structural = |position, of_type| Self::Structural(position, of_type);
        match (raw, name) {
            ("link", _) => Self::Link,
            ("visited", _) => Self::Visited,
            ("target-current", _) => Self::TargetCurrent,
            ("root", _) => Self::Root,
            ("empty", _) => Self::Empty,
            ("hover" | "active", _) => Self::UserAction,
            (_, "first-child") => structural(Position::First, false),
            (_, "last-child") => structural(Position::Last, false),
            (_, "only-child") => structural(Position::Only, false),
            (_, "nth-child") => structural(Position::Nth, false),
            (_, "nth-last-child") => structural(Position::NthLast, false),
            (_, "first-of-type") => structural(Position::First, true),
            (_, "last-of-type") => structural(Position::Last, true),
            (_, "only-of-type") => structural(Position::Only, true),
            (_, "nth-of-type") => structural(Position::Nth, true),
            (_, "nth-last-of-type") => structural(Position::NthLast, true),
            _ => Self::Other,
        }
    }
}

/// Identity of a `:has()` argument text: the key of the marks that `:has()`
/// matching leaves on anchors.
pub fn has_argument_id(arg: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    arg.hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests;

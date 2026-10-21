//! Исходы разбора объявлений для запросов возможностей, независимо от вычисленных умолчаний.

use super::Computed;
use crate::document::state::ValueCx;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ParseResult {
    #[default]
    Inactive,
    Accepted,
    Invalid,
    Unsupported,
}

/// CSS Cascade 5 §7.3: the keywords every property accepts in addition to its
/// own grammar (`initial`, `inherit`, `unset`, `revert`, `revert-layer`).
pub(crate) fn css_wide_keyword(value: &str) -> bool {
    ["initial", "inherit", "unset", "revert", "revert-layer"]
        .iter()
        .any(|keyword| value.trim().eq_ignore_ascii_case(keyword))
}

impl Computed {
    pub(crate) fn declaration_result(key: &str, value: &str, vcx: ValueCx<'_>) -> ParseResult {
        if !Self::recognizes_property(key, vcx) {
            return ParseResult::Unsupported;
        }
        // CSS Cascade 5 §7.3: CSS-wide keywords belong to every known property.
        if css_wide_keyword(value) {
            return ParseResult::Accepted;
        }
        let mut parser = Self {
            declaration_parse: ParseResult::Invalid,
            ..Self::default()
        };
        parser.apply_one(key, value, vcx);
        parser.declaration_parse
    }

    pub(crate) fn accept_declaration(&mut self) {
        if self.declaration_parse != ParseResult::Inactive {
            self.declaration_parse = ParseResult::Accepted;
        }
    }

    pub(crate) fn note_parsed<T>(&mut self, value: &Option<T>) {
        if value.is_some() {
            self.accept_declaration();
        }
    }
}

// Observe the final optional parse result, before storing it. The normal cascade
// leaves the observer inactive; parsing is evaluated exactly once in both modes.
macro_rules! record_parsed {
    ($parser:expr, $value:expr) => {{
        let parsed = $value;
        $parser.note_parsed(&parsed);
        parsed
    }};
}

// Use only after a grammar has succeeded, including valid none/auto values
// whose representation does not contain an optional parsed value.
macro_rules! accept_parsed {
    ($parser:expr, $value:expr) => {{
        let parsed = $value;
        $parser.accept_declaration();
        parsed
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acceptance_does_not_depend_on_the_parsed_value() {
        let mut parser = Computed {
            declaration_parse: ParseResult::Invalid,
            ..Computed::default()
        };
        parser.note_parsed(&None::<bool>);
        assert_eq!(parser.declaration_parse, ParseResult::Invalid);
        parser.note_parsed(&Some(false));
        assert_eq!(parser.declaration_parse, ParseResult::Accepted);
        parser.declaration_parse = ParseResult::Invalid;
        parser.note_parsed(&Some(0));
        assert_eq!(parser.declaration_parse, ParseResult::Accepted);
        parser.declaration_parse = ParseResult::Inactive;
        parser.note_parsed(&Some(0));
        assert_eq!(parser.declaration_parse, ParseResult::Inactive);
    }
}

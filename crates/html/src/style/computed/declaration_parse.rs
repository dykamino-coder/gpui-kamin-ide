//! Declaration parser outcomes for capability queries, independent of computed defaults.

use super::Computed;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ParseResult {
    #[default]
    Inactive,
    Accepted,
    Invalid,
    Unsupported,
}

impl Computed {
    pub(crate) fn declaration_result(key: &str, value: &str) -> ParseResult {
        if !Self::recognizes_property(key) {
            return ParseResult::Unsupported;
        }
        // CSS Cascade 5 §7.3: CSS-wide keywords belong to every known property.
        if ["initial", "inherit", "unset", "revert", "revert-layer"]
            .iter()
            .any(|keyword| value.eq_ignore_ascii_case(keyword))
        {
            return ParseResult::Accepted;
        }
        let mut parser = Self {
            declaration_parse: ParseResult::Invalid,
            ..Self::default()
        };
        parser.apply_one(key, value);
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

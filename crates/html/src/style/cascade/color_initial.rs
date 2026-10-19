//! Preserve color initial until the document can resolve the quirks table rule.

use crate::document::state::ValueCx;
use crate::style::computed::Computed;
use crate::style::values::value::Color;

pub(super) fn apply(c: &mut Computed, key: &str, value: &str, vcx: ValueCx<'_>) -> bool {
    if key != "color" {
        return false;
    }
    if value.eq_ignore_ascii_case("initial") {
        c.color_initial = true;
        c.color = Color::parse("black", vcx);
        return true;
    }
    if value.eq_ignore_ascii_case("inherit") || value.eq_ignore_ascii_case("currentcolor") {
        c.accept_declaration();
        c.color_initial = false;
        c.color = None;
        return true;
    }
    if Color::parse(value, vcx).is_some() {
        c.color_initial = false;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inherited_color_replaces_initial_but_invalid_color_preserves_it() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut c = Computed::default();
        c.apply_one("color", "initial", vcx);
        c.apply_one("color", "not-a-color", vcx);
        assert!(c.color_initial);
        for value in ["inherit", "currentColor", "unset"] {
            c.apply_one("color", "initial", vcx);
            c.apply_one("color", value, vcx);
            assert!(!c.color_initial);
            assert_eq!(c.color, None);
        }
        c.apply_one("color", "initial", vcx);
        c.apply_one("color", "red", vcx);
        assert!(!c.color_initial);
        assert_eq!(c.color, Color::parse("red", vcx));
    }
}

//! Каскад: `color: initial` помечается флагом `color_initial` и держится до
//! момента, когда документ сможет разрешить правило цвета таблиц в quirks.
//! `inherit`/`currentcolor` и любой разобранный цвет снимают пометку;
//! неразобранное значение её сохраняет.

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
        c.color_function = None;
        return true;
    }
    if value.eq_ignore_ascii_case("inherit") || value.eq_ignore_ascii_case("currentcolor") {
        c.accept_declaration();
        c.color_initial = false;
        c.color = None;
        c.color_function = None;
        return true;
    }
    if crate::style::values::value::StyleColor::parse_value(value, vcx).is_some() {
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

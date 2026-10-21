//! Тесты проверок возможностей для объявлений: принятые, недопустимые и неподдерживаемые исходы.

use crate::style::computed::{Computed, ParseResult};

#[test]
fn declaration_outcomes_include_values_equal_to_the_initial_state() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    for (property, value) in [
        ("column-count", "auto"),
        ("column-width", "auto"),
        ("container-type", "normal"),
        ("content-visibility", "visible"),
        ("will-change", "auto"),
        ("font-kerning", "auto"),
        ("font-stretch", "normal"),
        ("color", "transparent"),
        ("opacity", "0"),
        ("float", "none"),
        ("scrollbar-color", "auto"),
        ("scroll-snap-type", "none"),
        ("display", "block"),
        ("grid-template-columns", "none"),
        ("width", "auto"),
        ("border-style", "none"),
        ("border-color", "currentcolor"),
        ("border-top-color", "currentcolor"),
        ("border-inline", "1px solid red"),
        ("background-image", "none"),
        ("background-position", "center"),
        ("mask-image", "none"),
        ("mask-size", "auto"),
        ("white-space", "normal"),
        ("direction", "ltr"),
        ("unicode-bidi", "normal"),
        ("text-decoration", "none"),
        ("ruby-align", "space-around"),
        ("text-shadow", "none"),
        ("text-indent", "0px"),
        ("tab-size", "0"),
        ("quotes", "auto"),
        ("list-style", "none"),
        ("counter-reset", "none"),
        ("transform", "none"),
        ("filter", "none"),
        ("animation", "none"),
        ("d", "none"),
    ] {
        assert_eq!(
            Computed::declaration_result(property, value, vcx),
            ParseResult::Accepted,
            "{property}: {value}"
        );
        assert!(crate::style::css::supports(
            &format!("({property}: {value})"),
            vcx
        ));
    }
}

#[test]
fn invalid_values_and_unsupported_names_have_distinct_results() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    assert_eq!(
        Computed::declaration_result("definitely-not-a-property", "initial", vcx),
        ParseResult::Unsupported
    );
    for (property, value) in [
        ("color", "rainbow"),
        ("width", "nonsense"),
        ("font-stretch", "123"),
        ("font-size", "-1px"),
        ("column-count", "0"),
        ("container-type", "anchored"),
    ] {
        assert_eq!(
            Computed::declaration_result(property, value, vcx),
            ParseResult::Invalid,
            "{property}: {value}"
        );
        assert!(!crate::style::css::supports(
            &format!("({property}: {value})"),
            vcx
        ));
    }
}

#[test]
fn cascade_bookkeeping_does_not_accept_an_invalid_declaration() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut parser = Computed {
        declaration_parse: ParseResult::Invalid,
        decl_seq: 42,
        ..Computed::default()
    };
    parser.apply_one("color", "rainbow", vcx);
    assert_eq!(parser.declaration_parse, ParseResult::Invalid);
    assert_eq!(parser.decl_seq, 43);
    for value in ["initial", "inherit", "unset", "revert", "revert-layer"] {
        assert_eq!(
            Computed::declaration_result("color", value, vcx),
            ParseResult::Accepted
        );
    }
}

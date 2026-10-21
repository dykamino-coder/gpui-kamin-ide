//! Невалидные декларации сохраняют ранее вычисленные значения и связанные поля.

use crate::dom::{Element, Node};
use crate::style::computed::Computed;
use crate::style::css::parse_decls;
use crate::style::values::value::Len;

fn find<'a>(nodes: &'a [Node], id: &str) -> Option<&'a Element> {
    for n in nodes {
        if let Node::Element(e) = n {
            if e.attr("id") == Some(id) {
                return Some(e);
            }
            if let Some(f) = find(&e.children, id) {
                return Some(f);
            }
        }
    }
    None
}

fn styled(html: &str, id: &str) -> Computed {
    let doc = crate::document::Document::new(html, "");
    (*find(doc.nodes(), id).expect("element").style).clone()
}

fn one(css: &str) -> Computed {
    let st = crate::document::state::DocState::new(crate::style::css::Media::default());
    let mut c = Computed::default();
    c.apply_decls_with_vars(&parse_decls(css), &Default::default(), st.values());
    c
}

#[test]
fn f04_invalid_width_keeps_fit_content_arg() {
    let c = one("width: fit-content(40px); width: garbage");
    assert!(
        c.fit_arg[0].is_some(),
        "fit-content(40px) потерян невалидным width"
    );
}

#[test]
fn f04_invalid_padding_keeps_previous_padding() {
    let c = one("padding: 10px; padding: garbage");
    assert_eq!(
        c.padding.top,
        Some(Len::Px(10.0)),
        "invalid padding стёр padding"
    );
}

#[test]
fn f04_invalid_margin_keeps_previous_margin() {
    let c = one("margin: 10px; margin: garbage");
    assert_eq!(
        c.margin.top,
        Some(Len::Px(10.0)),
        "invalid margin стёр margin"
    );
}

#[test]
fn f04_invalid_margin_side_keeps_previous_side() {
    let c = one("margin-top: 7px; margin-top: garbage");
    assert_eq!(
        c.margin.top,
        Some(Len::Px(7.0)),
        "invalid margin-top стёр сторону"
    );
}

#[test]
fn f04_invalid_padding_in_document() {
    let c = styled(
        "<div id=d style='padding:10px;padding:garbage'>x</div>",
        "d",
    );
    assert_eq!(c.padding.top, Some(Len::Px(10.0)));
}

#[test]
fn f05_box_sizing_invalid_keyword_is_ignored() {
    let c = one("box-sizing: border-box; box-sizing: banana");
    assert_eq!(c.border_box, Some(true), "banana сбросил border-box");
}

#[test]
fn f05_box_sizing_invalid_keyword_is_not_supported() {
    let st = crate::document::state::DocState::new(crate::style::css::Media::default());
    let r = Computed::declaration_result("box-sizing", "banana", st.values());
    assert_eq!(
        format!("{r:?}"),
        "Invalid",
        "@supports(box-sizing: banana) истинно"
    );
}

#[test]
fn invalid_value_preserves_computed_for_every_registered_property() {
    let st = crate::document::state::DocState::new(crate::style::css::Media::default());
    let mut seed = Computed::default();
    for prop in crate::coverage::PROPERTIES {
        seed.apply_one(prop.name, prop.sample, st.values());
    }
    let mut failures = Vec::new();
    for prop in crate::coverage::PROPERTIES {
        for invalid in [
            "invalid()",
            "invalid(0)",
            "0 invalid()",
            "invalid() 0",
            "initial invalid()",
        ] {
            if Computed::declaration_result(prop.name, invalid, st.values())
                == crate::style::computed::ParseResult::Accepted
            {
                failures.push(format!("@supports({}: {invalid})", prop.name));
            }
            for mut c in [Computed::default(), seed.clone()] {
                c.apply_one(prop.name, prop.sample, st.values());
                let before = format!("{c:?}");
                let seq = c.decl_seq;
                c.apply_one(prop.name, invalid, st.values());
                // Счётчик входных деклараций сам по себе не является значением свойства.
                c.decl_seq = seq;
                if before != format!("{c:?}") {
                    failures.push(format!("{}: {invalid}", prop.name));
                    break;
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "invalid declaration changed Computed or was accepted: {failures:?}"
    );
}

#[test]
fn f05_supports_rejects_invalid_box_sizing_in_stylesheet() {
    let c = styled(
        "<style>#d { width: 7px } @supports (box-sizing: banana) { #d { width: 99px } }</style><div id=d>x</div>",
        "d",
    );
    assert_eq!(c.width, Some(Len::Px(7.0)));
    let st = crate::document::state::DocState::new(crate::style::css::Media::default());
    assert_eq!(
        format!(
            "{:?}",
            Computed::declaration_result("box-sizing", "BORDER-BOX", st.values())
        ),
        "Accepted"
    );
}

#[test]
fn invalid_component_rejects_the_entire_shorthand() {
    let st = crate::document::state::DocState::new(crate::style::css::Media::default());
    for (key, valid, invalid) in [
        ("padding", "7px", "20px banana"),
        ("overflow", "visible", "hidden invalid()"),
        ("padding", "7px", "-1px"),
        ("margin", "7px", "20px invalid()"),
        ("padding-inline", "7px", "20px invalid()"),
        ("gap", "7px", "20px invalid()"),
        ("border-spacing", "7px", "20px invalid()"),
        ("flex", "2 1 20px", "3 invalid()"),
        ("background-size", "7px", "20px invalid()"),
        ("border", "2px solid red", "3px banana"),
        ("outline", "2px solid red", "3px banana"),
        ("filter", "blur(2px)", "blur(4px) invalid()"),
        ("transform", "scale(2)", "scale(3) invalid()"),
        ("transition", "opacity 2s", "opacity 3s invalid()"),
        ("animation-duration", "2s", "banana"),
        ("contain-intrinsic-size", "7px", "20px invalid()"),
        ("background-image", "url(a), url(b)", "url(c), invalid()"),
        ("background-repeat", "repeat, no-repeat", "space, invalid()"),
        ("background-size", "7px, 9px", "20px, invalid()"),
        ("background", "red", "url(a) invalid()"),
        ("background-image", "url(a)", "url(b) invalid()"),
    ] {
        let mut c = Computed::default();
        c.apply_one(key, valid, st.values());
        let seq = c.decl_seq;
        let before = format!("{c:?}");
        c.apply_one(key, invalid, st.values());
        c.decl_seq = seq;
        assert_eq!(format!("{c:?}"), before, "{key}: {invalid}");
        assert_ne!(
            Computed::declaration_result(key, invalid, st.values()),
            crate::style::computed::ParseResult::Accepted,
            "@supports({key}: {invalid})"
        );
    }
}

#[test]
fn valid_reset_is_distinct_from_invalid_value() {
    assert!(one("background: currentcolor").background_rcs.is_some());
    let c = one(
        "margin: 9px; margin: auto; max-width: 20px; max-width: none; text-combine-upright: all; text-combine-upright: none",
    );
    assert_eq!(c.margin.top, Some(Len::Auto));
    assert!(c.max_width.is_none());
    assert!(c.combine_upright.is_none());
}

#[test]
fn valid_inherit_clears_local_inherited_lengths() {
    let c = one(
        "font-size: 20px; font-size: inherit; line-height: 1em; line-height: inherit; letter-spacing: 3px; letter-spacing: inherit; word-spacing: 3px; word-spacing: inherit",
    );
    assert_eq!(c.font_size, None);
    assert_eq!(c.line_height, None);
    assert_eq!(c.letter_spacing, None);
    assert_eq!(c.word_spacing, None);
}

#[test]
fn invalid_declaration_preserves_background_lists_and_flow_alignment() {
    let st = crate::document::state::DocState::new(crate::style::css::Media::default());
    for (key, valid) in [
        ("background-image", "url(a.png), url(b.png)"),
        ("background-repeat", "repeat, no-repeat"),
        ("align-self", "flow-end"),
    ] {
        let mut c = Computed::default();
        c.apply_one(key, valid, st.values());
        let before = format!("{c:?}");
        let seq = c.decl_seq;
        c.apply_one(key, "invalid()", st.values());
        c.decl_seq = seq;
        assert_eq!(format!("{c:?}"), before, "{key}");
    }
}

//! Тесты реестра: имена и начальные значения длинных свойств, раскрытие
//! сокращений при разборе, общие слова и значения с подстановкой.

use super::*;
use crate::style::css::{DeclBlock, parse_decls, parse_decls_in};

fn doc() -> std::rc::Rc<crate::document::state::DocState> {
    crate::document::state::DocState::new(crate::style::css::Media::default())
}

fn pairs(block: &DeclBlock) -> Vec<(String, String, u32)> {
    block
        .iter()
        .map(|d| (d.name.clone(), d.value.clone(), d.pos))
        .collect()
}

#[test]
fn every_longhand_round_trips_and_accepts_its_initial_value() {
    let doc = doc();
    let vcx = doc.values();
    for &id in LonghandId::ALL {
        assert_eq!(LonghandId::from_name(id.name()), Some(id));
        assert!(
            id.accepts(id.initial(), vcx),
            "{}: {}",
            id.name(),
            id.initial()
        );
        assert!(!id.inherited(), "{}", id.name());
        assert!(ShorthandId::from_name(id.name()).is_none());
    }
    assert_eq!(LonghandId::from_name("margin"), None);
    assert_eq!(LonghandId::from_name("color"), None);
}

#[test]
fn shorthands_cover_their_longhands_once() {
    for (name, count) in [
        ("margin", 4),
        ("padding", 4),
        ("inset", 4),
        ("margin-block", 2),
        ("margin-inline", 2),
        ("padding-block", 2),
        ("padding-inline", 2),
        ("inset-block", 2),
        ("inset-inline", 2),
    ] {
        let s = ShorthandId::from_name(name).expect(name);
        assert_eq!(s.name(), name);
        let mut ids = s.longhands().to_vec();
        ids.dedup();
        assert_eq!(ids.len(), count, "{name}");
    }
}

#[test]
fn four_side_shorthand_expands_by_value_count() {
    let doc = doc();
    let vcx = doc.values();
    for (value, expected) in [
        ("1px", ["1px", "1px", "1px", "1px"]),
        ("1px 2px", ["1px", "2px", "1px", "2px"]),
        ("1px 2px 3px", ["1px", "2px", "3px", "2px"]),
        (
            "1px calc(2px + 1%) 3px 4px",
            ["1px", "calc(2px + 1%)", "3px", "4px"],
        ),
    ] {
        let block = parse_decls_in(&format!("color: red; margin: {value}; top: 0"), vcx);
        let got: Vec<_> = pairs(&block)[1..5].to_vec();
        let names = ["margin-top", "margin-right", "margin-bottom", "margin-left"];
        for (i, (name, text, pos)) in got.into_iter().enumerate() {
            assert_eq!(
                (name.as_str(), text.as_str(), pos),
                (names[i], expected[i], 1)
            );
        }
        assert_eq!(block.0[5].pos, 2, "{value}");
    }
}

#[test]
fn axis_shorthands_expand_to_logical_longhands() {
    let doc = doc();
    let vcx = doc.values();
    let block = parse_decls_in("padding-inline: 1px 2px; inset-block: auto", vcx);
    assert_eq!(
        pairs(&block),
        [
            ("padding-inline-start".into(), "1px".into(), 0),
            ("padding-inline-end".into(), "2px".into(), 0),
            ("inset-block-start".into(), "auto".into(), 1),
            ("inset-block-end".into(), "auto".into(), 1),
        ]
    );
}

#[test]
fn invalid_shorthand_or_longhand_adds_nothing() {
    let doc = doc();
    let vcx = doc.values();
    for css in [
        "margin: 1px 2px 3px 4px 5px",
        "margin: garbage",
        "padding: 1px -2px",
        "padding: auto",
        "margin: inherit 1px",
        "margin-inline: 1px 2px 3px",
        "padding-left: -1px",
        "margin-top: anchor(--a top)",
        "width: none",
    ] {
        assert!(parse_decls_in(css, vcx).is_empty(), "{css}");
    }
    // Without a document only the shape of the value is checked.
    assert_eq!(parse_decls("margin: garbage").len(), 4);
    assert!(parse_decls("margin: 1px 2px 3px 4px 5px").is_empty());
}

#[test]
fn wide_keywords_and_substitutions_reach_every_longhand() {
    let doc = doc();
    let vcx = doc.values();
    let block = parse_decls_in(
        "margin: INHERIT !important; inset-inline: var(--a) 2px",
        vcx,
    );
    let kinds: Vec<_> = block.iter().map(|d| (d.specified, d.important)).collect();
    assert_eq!(kinds[..4], [(Specified::Wide(CssWide::Inherit), true); 4]);
    let pending = Specified::Pending(Some(ShorthandId::InsetInline));
    assert_eq!(kinds[4..], [(pending, false); 2]);
    assert_eq!(block.0[4].value, "var(--a) 2px");
    assert_eq!(block.0[4].id, Some(LonghandId::InsetInlineStart));
    assert_eq!(
        Specified::of("revert-layer"),
        Specified::Wide(CssWide::RevertLayer)
    );
    assert_eq!(Specified::of("attr(x px)"), Specified::Pending(None));
    assert_eq!(Specified::of("1px"), Specified::Value);
}

#[test]
fn substituted_shorthand_yields_the_component_of_each_longhand() {
    let doc = doc();
    let vcx = doc.values();
    let s = ShorthandId::Margin;
    assert_eq!(
        s.component_of(LonghandId::MarginLeft, "1px 2px 3px", vcx)
            .as_deref(),
        Some("2px")
    );
    assert_eq!(
        s.component_of(LonghandId::MarginLeft, "1px bogus", vcx),
        None
    );
    assert_eq!(s.component_of(LonghandId::MarginLeft, "inherit", vcx), None);
}

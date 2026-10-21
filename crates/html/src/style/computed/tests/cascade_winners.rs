//! Тесты каскада по карте: один победитель на имя, повтор по месту в блоке,
//! откат `revert` к агенту, `revert-layer`, важный `style=""` ниже важного
//! агента, негодный победитель уступает прежнему кандидату.

use crate::style::computed::Computed;
use crate::style::css::{DeclBlock, Origin, Rule, Selector, parse_decls};
use crate::style::values::value::{Color, Len};

fn rule(css: &str, origin: Origin, layer: Vec<u32>, order: usize) -> Rule {
    Rule {
        sel: Selector::parse("p").unwrap(),
        decls: parse_decls(css),
        order,
        origin,
        layer,
    }
}

#[test]
fn repeated_property_wins_at_its_own_position() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut c = Computed::default();
    c.apply_decls(
        &parse_decls("background: red; background-color: lime; background: blue"),
        vcx,
    );
    assert_eq!(c.background, Color::parse("blue", vcx));
}

#[test]
fn invalid_winner_yields_to_the_previous_candidate() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut c = Computed::default();
    c.apply_decls(&parse_decls("width: 10px; width: bogus"), vcx);
    assert_eq!(c.width, Some(Len::Px(10.0)));
    let a = rule("width: 20px", Origin::Author, vec![u32::MAX], 0);
    let b = rule("width: 1foo", Origin::Author, vec![u32::MAX], 1);
    let c = Computed::resolve(&[&a, &b], &DeclBlock::new(), vcx);
    assert_eq!(c.width, Some(Len::Px(20.0)));
}

#[test]
fn rollback_reaches_the_user_agent_and_the_previous_layer() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let ua = rule("width: 10px", Origin::UserAgent, vec![u32::MAX], 0);
    let author = rule("width: 20px", Origin::Author, vec![u32::MAX], 0);
    for (inline, expected) in [
        ("width: revert", 10.0),
        ("width: revert-layer", 20.0),
        ("width: 30px; width: revert !important", 10.0),
    ] {
        let c = Computed::resolve(&[&ua, &author], &parse_decls(inline), vcx);
        assert_eq!(c.width, Some(Len::Px(expected)), "{inline}");
    }
    let reverting = rule("width: revert", Origin::Author, vec![u32::MAX], 1);
    let c = Computed::resolve(&[&ua, &author, &reverting], &DeclBlock::new(), vcx);
    assert_eq!(c.width, Some(Len::Px(10.0)));
    let layered = rule("width: 40px", Origin::Author, vec![0, u32::MAX], 0);
    let rl = rule("width: revert-layer", Origin::Author, vec![u32::MAX], 1);
    let c = Computed::resolve(&[&ua, &layered, &rl], &DeclBlock::new(), vcx);
    assert_eq!(c.width, Some(Len::Px(40.0)));
}

#[test]
fn important_inline_is_below_important_user_agent() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let ua = rule(
        "color: green !important",
        Origin::UserAgent,
        vec![u32::MAX],
        0,
    );
    let c = Computed::resolve(&[&ua], &parse_decls("color: red !important"), vcx);
    assert_eq!(c.color, Color::parse("green", vcx));
}

#[test]
fn pseudo_layer_keeps_importance_across_rules() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let a = rule("color: green !important", Origin::Author, vec![u32::MAX], 0);
    let b = rule("color: red", Origin::Author, vec![u32::MAX], 1);
    let mut c = Computed::default();
    c.cascade_onto(
        &[&a, &b],
        &DeclBlock::new(),
        &Default::default(),
        crate::style::cascade::vars::CascadeCx::NONE,
        vcx,
    );
    assert_eq!(c.color, Color::parse("green", vcx));
}

#[test]
fn all_initial_resets_earlier_rules_but_not_later_ones() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let a = rule(
        "display: list-item; float: left; letter-spacing: 1em; width: 3px",
        Origin::Author,
        vec![u32::MAX],
        0,
    );
    let b = rule(
        "all: initial; width: 5px",
        Origin::Author,
        vec![u32::MAX],
        1,
    );
    let c = Computed::resolve(&[&a, &b], &DeclBlock::new(), vcx);
    let d = Computed::default();
    assert_eq!(c.float, d.float);
    assert_eq!(c.letter_spacing, Some(Len::Px(0.0)));
    assert_eq!(c.width, Some(Len::Px(5.0)));
    assert_ne!(c.display, Some(crate::style::computed::Display::ListItem));
}

#[test]
fn revert_without_agent_rule_keeps_the_implicit_agent_default() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut c = Computed::default();
    c.apply_decls(&parse_decls("display: inline; display: revert"), vcx);
    assert_eq!(c.display, Computed::default().display);
    let mut c = Computed::default();
    c.apply_decls(&parse_decls("display: inline; display: revert-layer"), vcx);
    assert_eq!(c.display, Computed::default().display);
}

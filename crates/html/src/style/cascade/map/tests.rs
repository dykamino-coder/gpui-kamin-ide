//! Карта каскада: победитель на имя, откаты `revert`/`revert-layer`, `all`.

use super::{CascadeMap, Pick};
use crate::style::css::{DeclBlock, Origin, Rule, Selector, parse_decls};

fn rule(css: &str, origin: Origin, layer: Vec<u32>, order: usize) -> Rule {
    Rule {
        sel: Selector::parse("p").unwrap(),
        decls: parse_decls(css),
        order,
        origin,
        layer,
        link: crate::style::css::LinkMatch::Any,
    }
}

fn author(css: &str, order: usize) -> Rule {
    rule(css, Origin::Author, vec![u32::MAX], order)
}

fn ua(css: &str) -> Rule {
    rule(css, Origin::UserAgent, vec![u32::MAX], 0)
}

/// Значение победителя имени (`unset` для отката без цели).
fn win(map: &CascadeMap<'_>, name: &str) -> Option<String> {
    map.picks(name).next().map(|p| match p {
        Pick::Value(_, v) => v.to_string(),
        Pick::Unset(_) => "unset".into(),
    })
}

#[test]
fn repeat_in_block_wins_by_position() {
    let r = author("color: red; color: green", 0);
    let none = DeclBlock::new();
    let map = CascadeMap::collect(&[&r], &none);
    assert_eq!(win(&map, "color").as_deref(), Some("green"));
    // The losing repeat stays as a fallback.
    let all: Vec<_> = map.picks("color").collect();
    assert_eq!(all.len(), 2);
}

#[test]
fn important_beats_later_normal_and_inline_is_below_important_ua() {
    let a = author("color: green !important", 0);
    let b = author("color: red", 1);
    let none = DeclBlock::new();
    let map = CascadeMap::collect(&[&a, &b], &none);
    assert_eq!(win(&map, "color").as_deref(), Some("green"));
    let agent = ua("color: green !important");
    let inline = parse_decls("color: red !important");
    let map = CascadeMap::collect(&[&agent], &inline);
    assert_eq!(win(&map, "color").as_deref(), Some("green"));
    // Normal inline beats a layered or more specific normal author rule.
    let inline = parse_decls("color: green");
    let map = CascadeMap::collect(&[&b], &inline);
    assert_eq!(win(&map, "color").as_deref(), Some("green"));
}

#[test]
fn revert_rolls_back_to_user_agent() {
    let agent = ua("display: block");
    let a = author("display: flex", 0);
    let b = author("display: revert", 1);
    let none = DeclBlock::new();
    let map = CascadeMap::collect(&[&agent, &a, &b], &none);
    assert_eq!(win(&map, "display").as_deref(), Some("block"));
    // Presentational hints count as author for revert.
    let hint = rule("display: grid", Origin::PresHint, vec![], 0);
    let map = CascadeMap::collect(&[&agent, &hint, &b], &none);
    assert_eq!(win(&map, "display").as_deref(), Some("block"));
    // Nothing to revert to: unset.
    let map = CascadeMap::collect(&[&a, &b], &none);
    assert_eq!(win(&map, "display").as_deref(), Some("unset"));
    // Inline revert also reaches the user agent.
    let inline = parse_decls("display: revert");
    let map = CascadeMap::collect(&[&agent, &a], &inline);
    assert_eq!(win(&map, "display").as_deref(), Some("block"));
}

#[test]
fn revert_layer_rolls_back_one_layer() {
    let base = rule("width: 10px", Origin::Author, vec![0, u32::MAX], 0);
    let top = rule(
        "width: 20px; width: revert-layer",
        Origin::Author,
        vec![1, u32::MAX],
        1,
    );
    let none = DeclBlock::new();
    let map = CascadeMap::collect(&[&base, &top], &none);
    assert_eq!(win(&map, "width").as_deref(), Some("10px"));
    // Important revert-layer drops the normal and important levels of its
    // layer and of the later (weaker for important) layers.
    let later = rule("width: 30px !important", Origin::Author, vec![u32::MAX], 2);
    let imp = rule(
        "width: revert-layer !important",
        Origin::Author,
        vec![1, u32::MAX],
        3,
    );
    let map = CascadeMap::collect(&[&base, &top, &later, &imp], &none);
    assert_eq!(win(&map, "width").as_deref(), Some("10px"));
    // In style="" it reverts to the author rules.
    let inline = parse_decls("width: revert-layer");
    let map = CascadeMap::collect(&[&base], &inline);
    assert_eq!(win(&map, "width").as_deref(), Some("10px"));
    // Chained: the layer below reverts again to the user agent.
    let agent = ua("width: 5px");
    let mid = rule("width: revert", Origin::Author, vec![0, u32::MAX], 0);
    let map = CascadeMap::collect(&[&agent, &mid, &top], &none);
    assert_eq!(win(&map, "width").as_deref(), Some("5px"));
}

#[test]
fn all_competes_in_every_name() {
    let agent = ua("display: block; color: blue");
    let a = author("color: red; direction: rtl", 0);
    let b = author("all: revert; width: 1px", 1);
    let c = author("width: 2px", 2);
    let none = DeclBlock::new();
    let map = CascadeMap::collect(&[&agent, &a, &b, &c], &none);
    assert_eq!(win(&map, "color").as_deref(), Some("blue"));
    assert_eq!(win(&map, "display").as_deref(), Some("block"));
    assert_eq!(win(&map, "direction").as_deref(), Some("rtl"));
    assert_eq!(win(&map, "width").as_deref(), Some("2px"));
    // A later declaration of the same block beats `all`.
    assert!(
        map.all_picks().next().is_none(),
        "all: revert has no own value"
    );
    let d = author("all: initial; color: green", 3);
    let map = CascadeMap::collect(&[&agent, &a, &d], &none);
    assert_eq!(win(&map, "color").as_deref(), Some("green"));
    assert_eq!(win(&map, "display").as_deref(), Some("initial"));
    assert!(matches!(
        map.all_picks().next(),
        Some(Pick::Value(_, "initial"))
    ));
    // `all` accepts CSS-wide keywords only.
    let e = author("all: red", 4);
    let map = CascadeMap::collect(&[&agent, &e], &none);
    assert_eq!(win(&map, "color").as_deref(), Some("blue"));
}

#[test]
fn custom_properties_revert_through_the_same_history() {
    let a = rule("--x: 1px", Origin::Author, vec![0, u32::MAX], 0);
    let b = rule("--x: revert-layer", Origin::Author, vec![1, u32::MAX], 1);
    let none = DeclBlock::new();
    let map = CascadeMap::collect(&[&a, &b], &none);
    assert_eq!(win(&map, "--x").as_deref(), Some("1px"));
    let c = author("all: initial", 2);
    let map = CascadeMap::collect(&[&a, &c], &none);
    assert_eq!(win(&map, "--x").as_deref(), Some("1px"));
}

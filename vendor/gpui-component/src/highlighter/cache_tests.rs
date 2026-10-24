//! Regressions for empty capture budgets and mutable/replaced style resolvers.

use super::{HighlightItem, SyntaxHighlighter};
use crate::highlighter::{HighlightTheme, capture_cache::CAPACITY};
use gpui::{HighlightStyle, Hsla, rgb};
use gpui_base::input::HighlightStyleResolver;
use ropey::Rope;
use std::sync::atomic::{AtomicBool, Ordering};

struct Resolver(AtomicBool);

impl HighlightStyleResolver for Resolver {
    fn style(&self, _: &str) -> Option<HighlightStyle> {
        Some(HighlightStyle {
            color: Some(color(self.0.load(Ordering::Relaxed))),
            ..Default::default()
        })
    }
}

fn color(changed: bool) -> Hsla {
    rgb(if changed { 0x00ff00 } else { 0xff0000 }).into()
}

#[test]
fn empty_queries_obey_the_capture_budget() {
    let mut highlighter = SyntaxHighlighter::build_inert("text".into());
    highlighter.text = Rope::from_str(&" ".repeat(CAPACITY + 100));
    let resolver = Resolver(AtomicBool::new(false));
    for start in 0..CAPACITY + 100 {
        assert_eq!(
            highlighter.styles(&(start..start + 1), &resolver),
            vec![(start..start + 1, HighlightStyle::default())],
        );
        assert!(highlighter.styles_cache.len() <= CAPACITY);
    }
}

fn seeded_highlighter() -> SyntaxHighlighter {
    let mut highlighter = SyntaxHighlighter::build_inert("text".into());
    highlighter.text = Rope::from_str("x");
    highlighter.styles_cache.get_or_insert_with(
        (0, 1),
        || vec![HighlightItem::new(0..1, "keyword")],
        |item| item.name.len(),
    );
    highlighter
}

#[test]
fn captures_survive_but_colors_follow_mutable_resolver() {
    let highlighter = seeded_highlighter();
    let resolver = Resolver(AtomicBool::new(false));
    assert_eq!(
        highlighter.styles(&(0..1), &resolver)[0].1.color,
        Some(color(false))
    );
    resolver.0.store(true, Ordering::Relaxed);
    assert_eq!(
        highlighter.styles(&(0..1), &resolver)[0].1.color,
        Some(color(true))
    );
}

#[test]
fn replacing_resolver_in_the_same_slot_uses_new_colors() {
    let highlighter = seeded_highlighter();
    let mut resolver = Resolver(AtomicBool::new(false));
    assert_eq!(
        highlighter.styles(&(0..1), &resolver)[0].1.color,
        Some(color(false))
    );
    resolver = Resolver(AtomicBool::new(true));
    assert_eq!(
        highlighter.styles(&(0..1), &resolver)[0].1.color,
        Some(color(true))
    );
}

#[test]
fn structural_edit_invalidates_captures() {
    let mut highlighter = seeded_highlighter();
    highlighter.edit_tree(None, &Rope::from_str("y"));
    assert_eq!(highlighter.styles_cache.len(), 0);
    assert_eq!(
        highlighter.styles(&(0..1), &Resolver(AtomicBool::new(false))),
        vec![(0..1, HighlightStyle::default())],
    );
}

#[test]
fn text_update_invalidates_captures() {
    let mut highlighter = seeded_highlighter();
    assert!(highlighter.update(None, &Rope::from_str("y"), None));
    assert_eq!(highlighter.styles_cache.len(), 0);
    assert_eq!(
        highlighter.styles(&(0..1), &Resolver(AtomicBool::new(false))),
        vec![(0..1, HighlightStyle::default())],
    );
}

#[test]
fn switching_theme_resolves_existing_captures_with_new_colors() {
    let highlighter = seeded_highlighter();
    let dark = HighlightTheme::default_dark();
    let light = HighlightTheme::default_light();
    let dark_style = highlighter.styles(&(0..1), &*dark)[0].1;
    let light_style = highlighter.styles(&(0..1), &*light)[0].1;
    assert_eq!(dark_style, dark.style("keyword").unwrap());
    assert_eq!(light_style, light.style("keyword").unwrap());
    assert_ne!(dark_style.color, light_style.color);
    assert_eq!(highlighter.styles_cache.len(), 1);
}

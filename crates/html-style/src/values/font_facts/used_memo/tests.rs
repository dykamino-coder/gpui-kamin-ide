//! Used-font memo: equal inputs select once; any changed input selects again.

use super::*;
use crate::style::computed::Computed;
use std::cell::Cell;

fn key<'a>(font: &'a StyleGroup<InheritedFont>, family: &'a str, px: f32) -> UsedFontKey<'a> {
    UsedFontKey {
        font,
        language: None,
        orientation: FontOrientation::Horizontal,
        family,
        computed_px: px,
    }
}

fn used(px: f32) -> UsedFont {
    UsedFont::new(None, px, 1.0, None)
}

#[test]
fn equal_inputs_select_once_and_any_changed_input_selects_again() {
    let doc = DocumentValues::detached(Default::default());
    let style = Computed::default();
    let shared = style.clone();
    let calls = Cell::new(0);
    let select = |px| {
        calls.set(calls.get() + 1);
        used(px)
    };
    doc.used_font_memo(key(&style.inh.font, "a", 16.0), || select(16.0));
    let hit = doc.used_font_memo(key(&shared.inh.font, "a", 16.0), || select(16.0));
    assert_eq!((calls.get(), hit.size_px), (1, 16.0));
    doc.used_font_memo(key(&style.inh.font, "a", 20.0), || select(20.0));
    doc.used_font_memo(key(&style.inh.font, "b", 16.0), || select(16.0));
    assert_eq!(calls.get(), 3);
    doc.font_inputs_changed();
    doc.used_font_memo(key(&style.inh.font, "a", 16.0), || select(16.0));
    doc.font_registry_changed();
    doc.used_font_memo(key(&style.inh.font, "a", 16.0), || select(16.0));
    assert_eq!(calls.get(), 5);
}

#[test]
fn a_detached_font_group_is_a_different_input() {
    let doc = DocumentValues::detached(Default::default());
    let style = Computed::default();
    let mut other = style.clone();
    other.inh.font.font_weight = Some(700);
    let calls = Cell::new(0);
    for font in [&style.inh.font, &other.inh.font, &style.inh.font] {
        doc.used_font_memo(key(font, "a", 16.0), || {
            calls.set(calls.get() + 1);
            used(16.0)
        });
    }
    assert_eq!(calls.get(), 2);
}

#[test]
fn the_memo_stays_bounded() {
    let doc = DocumentValues::detached(Default::default());
    let style = Computed::default();
    for i in 0..(CAPACITY * 3) {
        doc.used_font_memo(key(&style.inh.font, "a", i as f32), || used(i as f32));
    }
    let entries = doc.font_facts.used.entries.take();
    assert_eq!(entries.len(), CAPACITY);
}

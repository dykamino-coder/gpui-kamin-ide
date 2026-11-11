//! Память шрифтов прогонов: ключ по унаследованным группам, базе и кеглю; границы сборки.

use super::*;
use std::cell::Cell as Count;

fn state() -> std::rc::Rc<crate::document::state::DocState> {
    crate::document::state::DocState::new(crate::style::css::Media::default())
}

fn face(weight: f32) -> RunFace {
    let mut font = gpui::font("a");
    font.weight = gpui::FontWeight(weight);
    (font, None)
}

fn lookup(
    memo: &RunFaces,
    style: &Computed,
    base: &gpui::Font,
    base_px: f32,
    vcx: ValueCx<'_>,
    derived: &Count<u32>,
) -> RunFace {
    memo.face(style, base, base_px, vcx, || {
        derived.set(derived.get() + 1);
        face(style.inh.font.font_weight.unwrap_or(400) as f32)
    })
}

#[test]
fn styles_sharing_inherited_groups_share_the_face() {
    let doc = state();
    let vcx = doc.frame_values();
    let (memo, derived, base) = (RunFaces::default(), Count::new(0), gpui::font("a"));
    let parent = Computed::default();
    // A child changing no inherited value keeps its parent's groups; a box-only change too.
    let mut child = parent.clone();
    child.decor_box = 7;
    let first = lookup(&memo, &parent, &base, 16.0, vcx, &derived);
    let second = lookup(&memo, &child, &base, 16.0, vcx, &derived);
    assert_eq!((derived.get(), first), (1, second));
}

#[test]
fn equal_values_in_separate_groups_share_the_face() {
    let doc = state();
    let vcx = doc.frame_values();
    let (memo, derived, base) = (RunFaces::default(), Count::new(0), gpui::font("a"));
    let (mut one, mut two) = (Computed::default(), Computed::default());
    one.inh.font.font_weight = Some(700);
    two.inh.font.font_weight = Some(700);
    lookup(&memo, &one, &base, 16.0, vcx, &derived);
    let hit = lookup(&memo, &two, &base, 16.0, vcx, &derived);
    assert_eq!((derived.get(), hit), (1, face(700.0)));
}

#[test]
fn another_font_flow_base_or_size_derives_anew() {
    let doc = state();
    let vcx = doc.frame_values();
    let (memo, derived, base) = (RunFaces::default(), Count::new(0), gpui::font("a"));
    let plain = Computed::default();
    lookup(&memo, &plain, &base, 16.0, vcx, &derived);
    let mut bold = plain.clone();
    bold.inh.font.font_weight = Some(700);
    assert_eq!(
        lookup(&memo, &bold, &base, 16.0, vcx, &derived),
        face(700.0)
    );
    let mut spaced = plain.clone();
    spaced.inh.text_flow.lang = Some("ja".into());
    lookup(&memo, &spaced, &base, 16.0, vcx, &derived);
    lookup(&memo, &plain, &gpui::font("b"), 16.0, vcx, &derived);
    lookup(&memo, &plain, &base, 20.0, vcx, &derived);
    assert_eq!(derived.get(), 5);
    // The first input is still remembered.
    lookup(&memo, &plain, &base, 16.0, vcx, &derived);
    assert_eq!(derived.get(), 5);
}

#[test]
fn another_document_or_font_registry_derives_anew() {
    let (doc, other) = (state(), state());
    let (memo, derived, base) = (RunFaces::default(), Count::new(0), gpui::font("a"));
    let style = Computed::default();
    lookup(&memo, &style, &base, 16.0, doc.frame_values(), &derived);
    lookup(&memo, &style, &base, 16.0, other.frame_values(), &derived);
    doc.frame_values().doc.font_registry_changed();
    lookup(&memo, &style, &base, 16.0, doc.frame_values(), &derived);
    assert_eq!(derived.get(), 3);
}

#[test]
fn the_memory_is_bounded_and_replaces_the_oldest() {
    let doc = state();
    let vcx = doc.frame_values();
    let (memo, derived, base) = (RunFaces::default(), Count::new(0), gpui::font("a"));
    let style = Computed::default();
    for size in 0..=CAPACITY {
        lookup(&memo, &style, &base, size as f32, vcx, &derived);
    }
    let kept = memo.entries.take();
    assert_eq!(kept.len(), CAPACITY);
    memo.entries.set(kept);
    // Size 0 was the oldest and is gone, size 1 is still there.
    lookup(&memo, &style, &base, 1.0, vcx, &derived);
    lookup(&memo, &style, &base, 0.0, vcx, &derived);
    assert_eq!(derived.get() as usize, CAPACITY + 2);
}

#[test]
fn a_build_starts_and_ends_with_an_empty_memory() {
    let doc = state();
    let vcx = doc.frame_values();
    let (memo, derived, base) = (RunFaces::default(), Count::new(0), gpui::font("a"));
    let style = Computed::default();
    assert!(memo.active().is_none());
    {
        let _build = memo.build();
        let active = memo.active().expect("a build is in progress");
        lookup(active, &style, &base, 16.0, vcx, &derived);
        // A nested document's build empties the memory and leaves the outer build active.
        drop(memo.build());
        assert!(memo.active().is_some());
        lookup(&memo, &style, &base, 16.0, vcx, &derived);
    }
    assert!(memo.active().is_none());
    assert!(memo.entries.take().is_empty());
    assert_eq!(derived.get(), 2);
}

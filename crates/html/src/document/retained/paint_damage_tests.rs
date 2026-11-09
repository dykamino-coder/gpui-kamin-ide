//! Paint-only damage of root-flow blocks (`layout::reuse::store::Damage`): a
//! record that the cascade sees only as a new colour keeps the recorded layout
//! of the offscreen blocks, while the blocks on the screen are built with the
//! new colour instead of replaying the recorded paint.

use super::reuse_tests::{reused, update};
use super::tests::{View, frame, open};
use gpui::{TestAppContext, VisualTestContext};

/// A standard document (wrappers unwrapped into the root style) whose class
/// on `html` changes the colour or the margin of every row through a rule.
fn page(class: &str, text: &str) -> String {
    let rows: String = (0..600)
        .map(|i| {
            let body = if i == 300 { text } else { "row" };
            format!("<div class=r>{body} {i}</div>")
        })
        .collect();
    format!(
        "<!doctype html><html class='{class}'><head><style>\
         html,body{{margin:0;padding:0;color:#000}} .r{{height:20px}} \
         html.paint body{{color:#123456}} html.paint .r{{background-color:#abcdef}} \
         html.layout .r{{margin-top:1px}}</style></head><body>{rows}</body></html>"
    )
}

fn replayed(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> u64 {
    cx.update(|_, cx| view.read(cx).doc.replayed_blocks())
}

/// Two settled builds of `page("", ..)`: the second records the blocks.
fn settled(cx: &mut TestAppContext) -> (gpui::Entity<View>, &mut VisualTestContext) {
    let (view, cx) = open(cx, &page("", "row"));
    frame(cx);
    update(&view, cx, &page("", "edited"));
    update(&view, cx, &page("", "row"));
    (view, cx)
}

#[gpui::test]
fn a_paint_only_class_record_keeps_the_layout_of_the_offscreen_blocks(cx: &mut TestAppContext) {
    let (view, cx) = settled(cx);
    let (_, skipped_before, _) = reused(&view, cx);
    let replayed_before = replayed(&view, cx);
    update(&view, cx, &page("paint", "row"));
    let (_, skipped, rebuilt) = reused(&view, cx);
    assert!(
        skipped - skipped_before > 400,
        "offscreen blocks of a recoloured flow were built: {}",
        skipped - skipped_before
    );
    assert_eq!(rebuilt, 0, "a recoloured block reached the screen skipped");
    assert_eq!(
        replayed(&view, cx),
        replayed_before,
        "a visible block replayed its paint of the old colour"
    );
}

#[gpui::test]
fn a_layout_class_record_skips_nothing(cx: &mut TestAppContext) {
    let (view, cx) = settled(cx);
    let (_, skipped_before, _) = reused(&view, cx);
    update(&view, cx, &page("layout", "row"));
    let (_, skipped, _) = reused(&view, cx);
    assert_eq!(skipped, skipped_before, "a block with a new margin kept its layout");
}

#[gpui::test]
fn a_recoloured_block_keeps_its_record_for_layout_only(cx: &mut TestAppContext) {
    use crate::layout::reuse::Damage;
    let (view, cx) = settled(cx);
    let store = cx.update(|_, cx| view.read(cx).doc.reuse.clone());
    let block = store
        .recorded()
        .into_iter()
        .find(|e| e.tag == "div")
        .expect("settled blocks were recorded");
    let damage = |e: &crate::dom::Element| store.same_tree(e).map(|(_, damage)| damage);
    assert_eq!(damage(&block), Some(Damage::None));
    let mut recoloured = block.clone();
    recoloured.style.used_mut().inh.ui.color = Some(crate::style::values::value::Color {
        r: 0.5,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    });
    assert_eq!(damage(&recoloured), Some(Damage::Paint));
    let mut moved = block;
    moved.style.used_mut().reset_box_layout.margin.top =
        Some(crate::style::values::value::Len::Px(1.0));
    assert_eq!(damage(&moved), None, "a new margin is layout damage");
}


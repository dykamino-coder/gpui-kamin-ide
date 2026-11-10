//! Paint-only damage of root-flow blocks (`layout::reuse::store::Damage`): a
//! record that the cascade sees only as a new colour keeps the recorded layout
//! of the offscreen blocks, while the blocks on the screen are built with the
//! new colour instead of replaying the recorded paint.

use super::reuse_tests::{reused, update};
use super::tests::{View, frame, open};
use gpui::{TestAppContext, VisualTestContext};

/// A standard document whose class on `html` changes the colour or the margin
/// of every row through a rule. The background of the root boxes keeps them as
/// boxes with decoration inputs, which a rebuild allocates anew.
pub(super) fn page(class: &str, text: &str) -> String {
    let rows: String = (0..600)
        .map(|i| {
            let body = if i == 300 { text } else { "row" };
            format!("<div class=r>{body} {i}</div>")
        })
        .collect();
    format!(
        "<!doctype html><html class='{class}'><head><style>\
         html,body{{margin:0;padding:0;background:#fff;color:#000}} .r{{height:20px}} \
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
    assert_eq!(
        skipped, skipped_before,
        "a block with a new margin kept its layout"
    );
}

#[gpui::test]
fn a_recoloured_block_keeps_its_record_for_layout_only(cx: &mut TestAppContext) {
    use crate::layout::reuse::Damage;
    let (view, cx) = settled(cx);
    let store = cx.update(|_, cx| view.read(cx).doc.reuse.clone());
    // The last row recorded is far below the screen: a recoloured block is
    // kept for its layout only there.
    let block = store
        .recorded()
        .into_iter()
        .filter(|e| e.tag == "div")
        .max_by_key(|e| e.node_id)
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

#[gpui::test]
fn a_restyle_compares_the_records_at_the_rebuild_and_drops_their_frame_tree(
    cx: &mut TestAppContext,
) {
    let (view, cx) = settled(cx);
    let store = cx.update(|_, cx| view.read(cx).doc.reuse.clone());
    let marks: Vec<_> = store.recorded().iter().map(|e| e.children.mark()).collect();
    assert!(marks.len() > 400, "settled blocks were recorded");
    let html = page("paint", "row");
    cx.update(|_, cx| view.update(cx, |v, _| v.doc.update(&html, "").unwrap()));
    // The records compared at the rebuild hold no block of a frame tree.
    assert!(store.recorded().is_empty(), "a record kept its frame block");
    frame(cx);
    let alive = marks.iter().filter(|m| m.is_alive()).count();
    assert_eq!(alive, 0, "the previous frame tree outlived the rebuild");
    let (_, skipped, rebuilt) = reused(&view, cx);
    assert!(
        skipped > 400 && rebuilt == 0,
        "skipped {skipped}, rebuilt {rebuilt}"
    );
}

#[gpui::test]
fn a_patch_after_a_restyle_drops_the_verdicts_of_the_rebuild(cx: &mut TestAppContext) {
    let (view, cx) = settled(cx);
    let (_, skipped_before, _) = reused(&view, cx);
    let store = cx.update(|_, cx| view.read(cx).doc.reuse.clone());
    let restyled = page("paint", "row");
    cx.update(|_, cx| view.update(cx, |v, _| v.doc.update(&restyled, "").unwrap()));
    // A patch of the restyled tree before any frame (`Document::commit`):
    // the verdicts taken at the rebuild no longer hold.
    store.patch();
    frame(cx);
    let (_, skipped, rebuilt) = reused(&view, cx);
    assert_eq!(
        skipped, skipped_before,
        "a block was skipped on a stale verdict"
    );
    assert_eq!(rebuilt, 0);
}

#[gpui::test]
fn the_frame_after_a_restyle_dropped_records_wraps_no_block_for_recording(cx: &mut TestAppContext) {
    let (view, cx) = settled(cx);
    let tracked = |view: &gpui::Entity<View>, cx: &mut VisualTestContext| {
        cx.update(|_, cx| view.read(cx).doc.tracked_blocks())
    };
    let before = tracked(&view, cx);
    // The visible rows are recoloured: their records go at the rebuild, and a
    // record made now would go at the next such restyle.
    update(&view, cx, &page("paint", "row"));
    assert_eq!(
        tracked(&view, cx),
        before,
        "a restyled frame wrapped blocks"
    );
    let (_, skipped, _) = reused(&view, cx);
    assert!(skipped > 0, "the offscreen records were dropped too");
}

/// A document of three rows, all on the screen, whose class on `html`
/// recolours `body`: every record is near, so each such restyle drops all.
fn near(class: &str) -> String {
    format!(
        "<!doctype html><html class='{class}'><head><style>\
         html,body{{margin:0;background:#fff;color:#000}} .r{{height:20px}} \
         html.paint body{{color:#123456}}</style></head>\
         <body><div class=r>a</div><div class=r>b</div><div class=r>c</div></body></html>"
    )
}

#[gpui::test]
fn restyles_that_drop_every_record_record_nothing_again(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, &near(""));
    frame(cx);
    update(&view, cx, &near("x"));
    let tracked = |view: &gpui::Entity<View>, cx: &mut VisualTestContext| {
        cx.update(|_, cx| view.read(cx).doc.tracked_blocks())
    };
    // The first restyle drops the records of the settled frame; the store
    // starts over, and the frames that keep following rebuilds wrap nothing.
    update(&view, cx, &near("paint"));
    let after = tracked(&view, cx);
    for class in ["", "paint", "", "paint"] {
        update(&view, cx, &near(class));
    }
    assert_eq!(
        tracked(&view, cx),
        after,
        "a restyled block was recorded again"
    );
}

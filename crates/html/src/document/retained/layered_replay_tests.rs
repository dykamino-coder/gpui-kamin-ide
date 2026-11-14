//! Replay of root-flow blocks with positioned boxes and own layers
//! (`layout::reuse::layered`): a block whose out-of-flow boxes have their
//! containing block inside it replays, one whose boxes go to a layer outside
//! it is built; either way the frame equals a fresh document's, layer order
//! included.

use super::reuse_tests::{settled, update};
use super::tests::{View, frame, open};
use gpui::{TestAppContext, VisualTestContext};

/// A flow-root body: `target`, an inert witness block and a paragraph the
/// edits change. `class` goes on `html`; `alt` recolours the body.
pub(super) fn page(class: &str, target: &str, text: &str) -> String {
    format!(
        "<!doctype html><html class='{class}'><head><style>\
         html,body{{margin:0;color:#000}} html.alt body{{color:#c00}} \
         .w{{height:20px;background:#0a0}}</style></head>\
         <body style='display:flow-root'>{target}<div class=w></div><p>{text}</p></body></html>"
    )
}

pub(super) fn replayed(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> u64 {
    cx.update(|_, cx| view.read(cx).doc.replayed_subtrees())
}

pub(super) fn scene(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|window, _| {
        let quads = window.painted_quads().into_iter().map(|q| format!("{q:?}"));
        let lines = window
            .painted_underlines()
            .into_iter()
            .map(|u| format!("{u:?}"));
        quads.chain(lines).collect()
    })
}

/// Blocks replayed by the frame after editing `target`'s page into `next`
/// (warmed with text edits first), its scene, and a fresh document's.
pub(super) fn edit(
    cx: &mut TestAppContext,
    target: &str,
    next: &str,
) -> (u64, Vec<String>, Vec<String>, Vec<String>) {
    let (view, vcx) = settled(cx, &page("", target, "one"), &page("", target, "two"));
    update(&view, vcx, &page("", target, "one"));
    update(&view, vcx, &page("", target, "two"));
    let before = scene(vcx);
    let start = replayed(&view, vcx);
    update(&view, vcx, next);
    // A rebuild that starts the store over counts from zero again.
    let now = replayed(&view, vcx);
    let count = now.checked_sub(start).unwrap_or(now);
    let after = scene(vcx);
    let (_, fresh) = open(cx, next);
    frame(fresh);
    (count, before, after, scene(fresh))
}

/// A relative block that makes no stacking context: its positioned children
/// paint in the root context, outside the block's own paint.
const LOOSE: &str = "<div style='position:relative;height:60px'>\
    <div style='position:absolute;left:4px;top:4px;width:20px;height:20px;background:#00f;\
    transform:rotate(3deg);opacity:0.5'></div>\
    <div style='position:absolute;left:10px;top:10px;width:20px;height:20px;background:#f00;z-index:2'></div>\
    <div style='position:absolute;left:14px;top:14px;width:20px;height:20px;background:#ff0;z-index:-1'></div>\
    </div>";

/// The same boxes under a block that makes a stacking context (z-index: 0):
/// every layer of theirs paints inside the block's step-8 paint.
fn boxes() -> String {
    LOOSE.replacen("position:relative;", "position:relative;z-index:0;", 1)
}

#[gpui::test]
fn a_block_holding_its_positioned_layers_replays_in_layer_order(cx: &mut TestAppContext) {
    let boxes = boxes();
    let (count, _, after, fresh) = edit(cx, &boxes, &page("", &boxes, "one"));
    assert_eq!(count, 2, "the positioned block and the witness replay");
    assert_eq!(
        after, fresh,
        "the replayed layers differ from a fresh build"
    );
}

#[gpui::test]
fn a_block_whose_layers_paint_in_the_context_outside_it_is_built(cx: &mut TestAppContext) {
    let (count, _, after, fresh) = edit(cx, LOOSE, &page("", LOOSE, "one"));
    assert_eq!(count, 1, "only the witness replays");
    assert_eq!(after, fresh);
}

#[gpui::test]
fn a_recoloured_block_of_positioned_layers_replays(cx: &mut TestAppContext) {
    let boxes = boxes();
    let (count, before, after, fresh) = edit(cx, &boxes, &page("alt", &boxes, "two"));
    assert_eq!(count, 2, "the positioned block and the witness replay");
    assert_eq!(after, fresh);
    assert_eq!(before, after, "nothing of them paints in the colour");
}

#[gpui::test]
fn a_fixed_box_replays_only_under_a_transform_inside_the_block(cx: &mut TestAppContext) {
    let fixed = "<div style='position:fixed;left:5px;top:5px;width:10px;height:10px;background:#f0f'></div>";
    let held = format!("<div style='height:30px;transform:translate(0)'>{fixed}</div>");
    let (count, _, after, fresh) = edit(cx, &held, &page("", &held, "one"));
    assert_eq!(count, 2, "the transformed block holds its fixed box");
    assert_eq!(after, fresh);
    let loose = format!("<div style='height:30px;position:relative'>{fixed}</div>");
    let (count, _, after, fresh) = edit(cx, &loose, &page("", &loose, "one"));
    assert_eq!(count, 1, "a fixed box placed by the viewport is built");
    assert_eq!(after, fresh);
}

#[gpui::test]
fn an_absolute_box_placed_outside_the_block_is_built(cx: &mut TestAppContext) {
    let loose = "<div style='height:30px'><div style='position:absolute;left:50px;top:50px;\
                 width:10px;height:10px;background:#f00'></div></div>";
    let (count, _, after, fresh) = edit(cx, loose, &page("", loose, "one"));
    assert_eq!(count, 1, "only the witness replays");
    assert_eq!(after, fresh);
}

#[gpui::test]
fn an_opacity_change_rebuilds_the_layer(cx: &mut TestAppContext) {
    let layer = |o: &str| {
        format!(
            "<div style='position:relative;height:30px'><div style='position:absolute;width:20px;\
             height:20px;background:#00f;opacity:{o}'></div></div>"
        )
    };
    let (count, before, after, fresh) = edit(cx, &layer("0"), &page("", &layer("1"), "two"));
    assert!(count <= 1, "the changed layer is built");
    assert_eq!(after, fresh);
    assert_ne!(before, after, "opacity 0 to 1 reached the frame");
}

#[gpui::test]
fn a_moved_paint_key_rebuilds_the_positioned_block_after_it(cx: &mut TestAppContext) {
    // The block before takes one more step-8 paint key after the edit: the
    // positioned block after it would paint with another key, so its record
    // does not fit (`ReplayRecord::fits`, start counters) and it is built.
    let rel = "<div style='position:relative;height:5px'></div>";
    let pair = |n: usize| {
        format!(
            "<div>{}</div><div style='position:relative;top:4px;height:20px;background:#00f'>\
             <div style='position:absolute;left:2px;top:2px;width:8px;height:8px;background:#f00'></div></div>",
            rel.repeat(n)
        )
    };
    let (count, _, after, fresh) = edit(cx, &pair(1), &page("", &pair(2), "one"));
    assert_eq!(count, 0, "every block after the change takes other keys");
    assert_eq!(
        after, fresh,
        "layer order after the moved keys differs from a fresh build"
    );
}

#[gpui::test]
fn a_positioned_block_keeps_its_key_when_the_block_before_keeps_its_count(cx: &mut TestAppContext) {
    let pair = |w: &str| {
        format!(
            "<div style='position:relative;height:5px'>{w}</div>\
             <div style='position:relative;top:-3px;height:20px;background:#00f'></div>"
        )
    };
    let (count, _, after, fresh) = edit(cx, &pair("a"), &page("", &pair("b"), "one"));
    assert_eq!(
        count, 2,
        "the positioned block after the change and the witness replay"
    );
    assert_eq!(after, fresh, "the replayed block keeps its step-8 order");
}

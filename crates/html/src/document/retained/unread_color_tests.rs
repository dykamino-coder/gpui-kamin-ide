//! Replay of root-flow blocks over a recolouring class edit
//! (`layout::reuse::unread_color`): a block no box of which paints in the
//! inherited colour replays its recorded frame, and a block that does is
//! built with the new colour. Either way the frame equals a fresh document's.

use super::reuse_tests::{settled, update};
use super::tests::{View, frame, open};
use gpui::{TestAppContext, VisualTestContext};

/// A flow-root body (no skip records, `replay_tests`): two inert witness
/// blocks around `target`, and a paragraph whose text the warm-up edits.
/// The class `alt` on `html` recolours the body.
fn page(class: &str, target: &str, text: &str) -> String {
    format!(
        "<!doctype html><html class='{class}'><head><style>\
         html,body{{margin:0;color:#000}} html.alt body{{color:#c00}} \
         .w{{height:20px;background:#0a0;border:1px solid #00f}}</style></head>\
         <body style='display:flow-root'><div class=w></div>{target}<div class=w></div>\
         <p>{text}</p></body></html>"
    )
}

fn replayed(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> u64 {
    cx.update(|_, cx| view.read(cx).doc.replayed_subtrees())
}

/// Quads and underlines of the shown frame in paint order.
fn scene(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|window, _| {
        let quads = window.painted_quads().into_iter().map(|q| format!("{q:?}"));
        let lines = window
            .painted_underlines()
            .into_iter()
            .map(|u| format!("{u:?}"));
        quads.chain(lines).collect()
    })
}

struct Recoloured {
    /// Blocks replayed by the recoloured frame.
    replayed: u64,
    before: Vec<String>,
    after: Vec<String>,
    fresh: Vec<String>,
}

/// Warm the replay records of `target`'s page, then recolour it.
fn recolour(cx: &mut TestAppContext, target: &str) -> Recoloured {
    let (view, vcx) = settled(cx, &page("", target, "one"), &page("", target, "two"));
    // The second settled build records the blocks for replay; the next replays.
    update(&view, vcx, &page("", target, "one"));
    update(&view, vcx, &page("", target, "two"));
    let witnesses = replayed(&view, vcx);
    update(&view, vcx, &page("", target, "one"));
    assert!(
        replayed(&view, vcx) - witnesses >= 2,
        "the witness blocks did not replay before the edit"
    );
    let before = scene(vcx);
    let start = replayed(&view, vcx);
    update(&view, vcx, &page("alt", target, "one"));
    let replayed = replayed(&view, vcx) - start;
    let after = scene(vcx);
    let (_, fresh) = open(cx, &page("alt", target, "one"));
    frame(fresh);
    Recoloured {
        replayed,
        before,
        after,
        fresh: scene(fresh),
    }
}

#[gpui::test]
fn a_block_that_paints_no_inherited_colour_replays_its_frame(cx: &mut TestAppContext) {
    let r = recolour(
        cx,
        "<div style='height:20px;border:1px solid #333;background:#ff0'>\
         <div style='height:5px;border-top:2px dashed #f0f'></div> </div>",
    );
    assert_eq!(r.replayed, 3, "the inert target and the witnesses replay");
    assert_eq!(
        r.after, r.fresh,
        "the replayed frame differs from a fresh one"
    );
    assert_eq!(r.before, r.after, "nothing it paints has the colour");
}

/// A target that paints in `currentcolor`: built anew, and its frame is a
/// fresh document's. `visible`: the colour is in a quad or an underline.
fn reads_colour(cx: &mut TestAppContext, target: &str, visible: bool) {
    let r = recolour(cx, target);
    assert_eq!(r.replayed, 2, "only the witnesses replay: {target}");
    assert_eq!(r.after, r.fresh, "the recoloured frame differs: {target}");
    if visible {
        assert_ne!(
            r.before, r.after,
            "the colour did not reach the frame: {target}"
        );
    }
}

#[gpui::test]
fn a_current_colour_border_is_repainted(cx: &mut TestAppContext) {
    reads_colour(cx, "<div style='height:20px;border:2px solid'></div>", true);
}

#[gpui::test]
fn a_current_colour_outline_is_repainted(cx: &mut TestAppContext) {
    reads_colour(
        cx,
        "<div style='height:20px;margin:4px;outline:2px solid'></div>",
        true,
    );
}

#[gpui::test]
fn a_current_colour_box_shadow_is_repainted(cx: &mut TestAppContext) {
    reads_colour(
        cx,
        "<div style='height:20px;box-shadow:2px 2px 0'></div>",
        false,
    );
}

#[gpui::test]
fn a_current_colour_svg_fill_and_stroke_are_repainted(cx: &mut TestAppContext) {
    reads_colour(
        cx,
        "<div><svg width='20' height='20'><rect width='20' height='20' fill='currentColor'/></svg></div>",
        false,
    );
    reads_colour(
        cx,
        "<div><svg width='20' height='20'><rect width='18' height='18' fill='none' \
         stroke='currentColor'/></svg></div>",
        false,
    );
}

#[gpui::test]
fn an_auto_coloured_text_decoration_is_repainted(cx: &mut TestAppContext) {
    reads_colour(cx, "<div style='text-decoration:underline'>t</div>", true);
}

#[gpui::test]
fn an_editable_box_and_a_control_are_repainted(cx: &mut TestAppContext) {
    reads_colour(cx, "<div contenteditable style='height:20px'></div>", false);
    reads_colour(cx, "<div><input type='checkbox' checked></div>", false);
}

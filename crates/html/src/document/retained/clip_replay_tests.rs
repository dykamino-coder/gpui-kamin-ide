//! Replay of box contents under a container's clip (`layout::reuse::replay`):
//! a content drawn under an `overflow` clip keeps its mask relative to its
//! layout parent and replays where the parent lies the same size, moved
//! with it; the frame equals a fresh document's.

use super::layered_replay_tests::scene;
use super::reuse_tests::update;
use super::tests::{View, frame, open};
use gpui::{ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext, point, px};

fn cx_replays(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> u64 {
    cx.update(|_, cx| view.read(cx).doc.replayed_contents())
}

/// Nine blocks in a clipping wrapper; the block `edited` grows by `rule`.
// Integral line heights keep moved glyphs on the same device-pixel phase.
fn clipped(edited: Option<usize>, wrap: &str, item: &str) -> String {
    let items: String = (0..9)
        .map(|i| {
            let mark = if Some(i) == edited { " data-x" } else { "" };
            format!("<div class=item{mark}><span>block {i}</span> <b>bold</b></div>")
        })
        .collect();
    format!(
        "<!doctype html><html><head><style>body{{margin:0;line-height:20px}} \
         #wrap{{{wrap}}} .item{{padding:2px;border:1px solid #333;{item}}} \
         [data-x]{{height:40px}}</style></head><body><p>top</p><div id=wrap>{items}</div></body></html>"
    )
}

/// The first block grows under the clip; the blocks below move inside it.
fn grow_and_compare(cx: &mut TestAppContext, wrap: &str, item: &str) -> u64 {
    let (view, vcx) = open(cx, &clipped(None, wrap, item));
    frame(vcx);
    update(&view, vcx, &clipped(Some(8), wrap, item));
    let start = cx_replays(&view, vcx);
    update(&view, vcx, &clipped(Some(0), wrap, item));
    let replays = cx_replays(&view, vcx) - start;
    let after = scene(vcx);
    let (_, fresh) = open(cx, &clipped(Some(0), wrap, item));
    frame(fresh);
    assert_eq!(
        after,
        scene(fresh),
        "the clipped replay differs from a fresh build"
    );
    replays
}

#[gpui::test]
fn contents_under_a_container_clip_replay_moved(cx: &mut TestAppContext) {
    let replays = grow_and_compare(cx, "overflow:hidden", "");
    // Blocks 0 and 8 changed; the others replay under the wrapper's clip.
    assert!(replays >= 7, "clipped contents replayed: {replays}");
}

#[gpui::test]
fn contents_under_nested_clips_replay_moved(cx: &mut TestAppContext) {
    let replays = grow_and_compare(cx, "overflow:hidden", "overflow:hidden");
    assert!(
        replays >= 7,
        "contents under nested clips replayed: {replays}"
    );
}

#[gpui::test]
fn contents_under_a_rounded_clip_replay_moved(cx: &mut TestAppContext) {
    let wrap = "overflow:hidden;border-radius:12px;border:3px solid #a00";
    let replays = grow_and_compare(cx, wrap, "border-radius:6px;overflow:hidden");
    assert!(
        replays >= 7,
        "contents under rounded clips replayed: {replays}"
    );
}

/// A short clip cuts the last blocks: the cut ones are built, the frame
/// still equals a fresh one.
#[gpui::test]
fn contents_cut_by_a_short_clip_match_a_fresh_build(cx: &mut TestAppContext) {
    grow_and_compare(cx, "overflow:hidden;height:150px", "");
}

fn scroll(cx: &mut VisualTestContext) {
    cx.simulate_event(ScrollWheelEvent {
        position: point(px(10.), px(40.)),
        delta: ScrollDelta::Pixels(point(px(0.), px(-37.))),
        ..Default::default()
    });
    frame(cx);
}

/// An edit inside a scrolled container: its contents lie under the
/// scrollport's clip at the scroll offset, as a fresh build lays them.
#[gpui::test]
fn contents_of_a_scrolled_container_match_a_fresh_build(cx: &mut TestAppContext) {
    let wrap = "overflow:auto;height:120px";
    let (view, vcx) = open(cx, &clipped(None, wrap, ""));
    frame(vcx);
    scroll(vcx);
    update(&view, vcx, &clipped(Some(8), wrap, ""));
    update(&view, vcx, &clipped(Some(3), wrap, ""));
    scroll(vcx);
    let after = scene(vcx);
    let (fresh, fcx) = open(cx, &clipped(None, wrap, ""));
    frame(fcx);
    scroll(fcx);
    update(&fresh, fcx, &clipped(Some(3), wrap, ""));
    scroll(fcx);
    assert_eq!(
        after,
        scene(fcx),
        "the scrolled replay differs from a fresh build"
    );
}

/// The wrapper's clip shrinks while the blocks inside stay: their copies
/// are clipped by the new clip, as a fresh build clips them.
#[gpui::test]
fn contents_under_a_shrunk_ancestor_clip_match_a_fresh_build(cx: &mut TestAppContext) {
    let page = |h: u32, edited: usize| {
        let outer = format!("<div style='overflow:hidden;height:{h}px'>");
        clipped(Some(edited), "overflow:hidden", "")
            .replace("<div id=wrap>", &format!("{outer}<div id=wrap>"))
            .replace("</div></body>", "</div></div></body>")
    };
    let (view, vcx) = open(cx, &page(400, 8));
    frame(vcx);
    update(&view, vcx, &page(400, 7));
    update(&view, vcx, &page(90, 7));
    let after = scene(vcx);
    let (_, fresh) = open(cx, &page(90, 7));
    frame(fresh);
    assert_eq!(
        after,
        scene(fresh),
        "a copy under a shrunk clip differs from a fresh build"
    );
    let _: &gpui::Entity<View> = &view;
}

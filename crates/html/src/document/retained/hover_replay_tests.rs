//! Replay of box contents with `:hover` styles (`layout::reuse::replay`): a
//! record notes the hover each of its hover boxes painted, and only a record
//! over a box the pointer entered or left is built again. Every frame equals
//! a fresh document's under the same pointer.

use super::layered_replay_tests::scene;
use super::reuse_tests::update;
use super::tests::{View, frame, open};
use gpui::{Modifiers, Pixels, Point, TestAppContext, VisualTestContext, point, px};

fn replays(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> u64 {
    cx.update(|_, cx| view.read(cx).doc.replayed_contents())
}

/// Nine items in a wrapper, each with a hover box 10px high; the last item
/// is `tail` px high: an edit of its height builds the wrapper again and
/// moves no other item. The hover box of item `i` lies at y 43 + 36 i to 53 + 36 i.
// Integral line heights keep every box on whole pixels.
fn items(tail: u32) -> String {
    items_with(tail, ".h:hover{background:green}")
}

fn items_with(tail: u32, hover: &str) -> String {
    let items: String = (0..9)
        .map(|i| format!("<div class=item><span>item {i}</span><div class=h></div></div>"))
        .collect();
    format!(
        "<!doctype html><html><head><style>body{{margin:0;line-height:20px}} \
         .item{{padding:2px;border:1px solid #333}} .h{{height:10px;background:#eee}} \
         {hover} .item:last-child{{height:{tail}px}}</style></head>\
         <body><p>top</p><div id=wrap>{items}</div></body></html>"
    )
}

fn hover_box(i: usize) -> Point<Pixels> {
    point(px(10.), px(48. + 36. * i as f32))
}

/// A fresh document's scene for `html` under the pointer at `pointer`.
fn fresh(cx: &mut TestAppContext, html: &str, pointer: Point<Pixels>) -> Vec<String> {
    let (_, fresh) = open(cx, html);
    fresh.simulate_mouse_move(pointer, None, Modifiers::default());
    frame(fresh);
    frame(fresh);
    scene(fresh)
}

/// Settle the items with the pointer at `pointer`, then edit the last one: the
/// contents replayed by the edit frame, the frame and a fresh one's.
fn edit_under(cx: &mut TestAppContext, pointer: Point<Pixels>) -> (u64, Vec<String>, Vec<String>) {
    let (view, vcx) = open(cx, &items(50));
    vcx.simulate_mouse_move(pointer, None, Modifiers::default());
    frame(vcx);
    update(&view, vcx, &items(60));
    update(&view, vcx, &items(60));
    let start = replays(&view, vcx);
    update(&view, vcx, &items(70));
    let replayed = replays(&view, vcx) - start;
    let after = scene(vcx);
    (replayed, after, fresh(cx, &items(70), pointer))
}

#[gpui::test]
fn contents_without_hover_styles_replay(cx: &mut TestAppContext) {
    let (view, vcx) = open(cx, &items_with(50, ""));
    frame(vcx);
    update(&view, vcx, &items_with(60, ""));
    update(&view, vcx, &items_with(60, ""));
    let start = replays(&view, vcx);
    update(&view, vcx, &items_with(70, ""));
    let replayed = replays(&view, vcx) - start;
    assert!(replayed >= 7, "contents replayed: {replayed}");
}

#[gpui::test]
fn contents_with_hover_styles_replay_away_from_the_pointer(cx: &mut TestAppContext) {
    let (replayed, after, fresh) = edit_under(cx, point(px(10.), px(500.)));
    assert_eq!(after, fresh, "the replayed frame differs from a fresh one");
    // The items above the edited one replay with their hover boxes.
    assert!(
        replayed >= 7,
        "contents with hover styles replayed: {replayed}"
    );
}

#[gpui::test]
fn a_content_under_the_pointer_replays_its_hover(cx: &mut TestAppContext) {
    let (replayed, after, fresh) = edit_under(cx, hover_box(4));
    assert_eq!(after, fresh, "the replayed hover differs from a fresh one");
    assert!(
        replayed >= 7,
        "contents replayed under the pointer: {replayed}"
    );
}

/// The pointer that enters, then leaves, a hover box of a settled frame:
/// only the item under it is built, the others replay, and each frame equals
/// a fresh one's under the same pointer.
#[gpui::test]
fn the_pointer_entering_a_hover_box_builds_only_its_content(cx: &mut TestAppContext) {
    let html = items(60);
    let steps = [
        ("enters item 4", hover_box(4)),
        ("moves to item 6", hover_box(6)),
        ("leaves", point(px(10.), px(500.))),
    ];
    let fresh: Vec<_> = steps.iter().map(|(_, p)| fresh(cx, &html, *p)).collect();
    let (view, vcx) = open(cx, &items(50));
    frame(vcx);
    update(&view, vcx, &html);
    update(&view, vcx, &html);
    for ((step, pointer), fresh) in steps.into_iter().zip(fresh) {
        let start = replays(&view, vcx);
        vcx.simulate_mouse_move(pointer, None, Modifiers::default());
        frame(vcx);
        let replayed = replays(&view, vcx) - start;
        assert_eq!(
            scene(vcx),
            fresh,
            "the pointer {step}: the frame differs from a fresh one"
        );
        assert!(
            replayed >= 7,
            "the pointer {step}: contents replayed {replayed}"
        );
    }
}

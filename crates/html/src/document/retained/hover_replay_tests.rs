//! Replay of box contents with `:hover` styles (`layout::reuse::replay`): a
//! record notes the hover each of its hover boxes painted, and only a record
//! over a box the pointer entered or left is built again. Every frame equals
//! a fresh document's under the same pointer.

use super::layered_replay_tests::scene;
use super::reuse_tests::update;
use super::tests::{View, frame, open};
use gpui::{Modifiers, Pixels, Point, TestAppContext, VisualTestContext, point, px};

const HOVER: &str = ".h:hover{background:green}";

fn replays(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> u64 {
    cx.update(|_, cx| view.read(cx).doc.replayed_contents())
}

/// Nine items in a wrapper, each led by a hover box 200px wide; the last
/// one is `last` px high, so its edit moves no other item. Item `i` lies at
/// y 20 + 26 i to 46 + 26 i.
fn items(last: u32, hover: &str) -> String {
    let items: String = (0..9)
        .map(|i| {
            let mark = if i == 8 { " data-x" } else { "" };
            format!("<div class=item{mark}><b class=h>hover</b> <span>item {i}</span></div>")
        })
        .collect();
    format!(
        "<!doctype html><html><head><style>body{{margin:0;line-height:20px}} \
         .item{{padding:2px;border:1px solid #333}} \
         .h{{display:inline-block;width:200px;background:#eee}} {hover} \
         [data-x]{{height:{last}px}}</style></head>\
         <body><p>top</p><div id=wrap>{items}</div></body></html>"
    )
}

/// A point on the hover box of item `i`.
fn hover_box(i: usize) -> Point<Pixels> {
    point(px(10.), px(33. + 26. * i as f32))
}

const AWAY: Point<Pixels> = Point {
    x: px(10.),
    y: px(500.),
};

/// `after` equals `fresh`; else the first differing entry fails.
fn same(after: &[String], fresh: &[String], what: &str) {
    let at = after.iter().zip(fresh).position(|(a, b)| a != b);
    if let Some(at) = at {
        panic!("{what} at {at}:\n{}\n{}", after[at], fresh[at]);
    }
    assert_eq!(after.len(), fresh.len(), "{what}: entries");
}

/// A fresh document's scene for `html` under the pointer at `pointer`.
fn fresh(cx: &mut TestAppContext, html: &str, pointer: Point<Pixels>) -> Vec<String> {
    let (_, fresh) = open(cx, html);
    fresh.simulate_mouse_move(pointer, None, Modifiers::default());
    frame(fresh);
    frame(fresh);
    scene(fresh)
}

/// Settle the items with the pointer at `pointer`, then grow the last one:
/// the contents replayed by the edit frame, the frame and a fresh one's.
fn grow_under(
    cx: &mut TestAppContext,
    pointer: Point<Pixels>,
    hover: &str,
) -> (u64, Vec<String>, Vec<String>) {
    let (view, vcx) = open(cx, &items(20, hover));
    vcx.simulate_mouse_move(pointer, None, Modifiers::default());
    frame(vcx);
    update(&view, vcx, &items(40, hover));
    let start = replays(&view, vcx);
    update(&view, vcx, &items(60, hover));
    let replayed = replays(&view, vcx) - start;
    let after = scene(vcx);
    (replayed, after, fresh(cx, &items(60, hover), pointer))
}

#[gpui::test]
fn contents_without_hover_styles_replay(cx: &mut TestAppContext) {
    let (replayed, after, fresh) = grow_under(cx, AWAY, "");
    same(
        &after,
        &fresh,
        "the replayed frame differs from a fresh one",
    );
    assert!(replayed >= 8, "contents replayed: {replayed}");
}

#[gpui::test]
fn contents_with_hover_styles_replay_away_from_the_pointer(cx: &mut TestAppContext) {
    let (replayed, after, fresh) = grow_under(cx, AWAY, HOVER);
    same(
        &after,
        &fresh,
        "the replayed frame differs from a fresh one",
    );
    // The items above the edited one replay with their hover boxes.
    assert!(
        replayed >= 8,
        "contents with hover styles replayed: {replayed}"
    );
}

/// The item under the pointer replays with the hover it shows.
#[gpui::test]
fn a_content_under_the_pointer_replays_its_hover(cx: &mut TestAppContext) {
    let (replayed, after, fresh) = grow_under(cx, hover_box(4), HOVER);
    same(
        &after,
        &fresh,
        "the replayed hover differs from a fresh one",
    );
    assert!(
        replayed >= 8,
        "contents replayed under the pointer: {replayed}"
    );
}

/// The pointer that enters a hover box, moves to another and leaves: each
/// frame equals a fresh one's under the same pointer, so a record whose hover
/// changed is not replayed.
#[gpui::test]
fn the_pointer_moving_over_hover_boxes_shows_their_hover(cx: &mut TestAppContext) {
    let html = items(40, HOVER);
    let steps = [
        ("enters item 4", hover_box(4)),
        ("moves to item 6", hover_box(6)),
        ("leaves", AWAY),
    ];
    let fresh: Vec<_> = steps.iter().map(|(_, p)| fresh(cx, &html, *p)).collect();
    let (view, vcx) = open(cx, &items(20, HOVER));
    frame(vcx);
    update(&view, vcx, &html);
    update(&view, vcx, &html);
    for ((step, pointer), fresh) in steps.into_iter().zip(fresh) {
        vcx.simulate_mouse_move(pointer, None, Modifiers::default());
        frame(vcx);
        assert_eq!(
            scene(vcx),
            fresh,
            "the pointer {step}: the frame differs from a fresh one"
        );
    }
}

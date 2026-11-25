//! A box with a `:hover` style is recorded again like any box: its own paint
//! is its style refined by the hover it shows (`gpui::Window::layer_box_hover`),
//! under the pointer or away from it, and the pointer that enters or leaves
//! it records it again (`Builder::repaint_hovers`). Each kept frame equals a
//! fresh document's under the same pointer.

use super::box_repaint_tests::{frames, quads};
use super::reuse_tests::{settled, update};
use super::tests::{frame, open};
use gpui::{Modifiers, Point, TestAppContext, point, px};

fn page(css: &str, body: &str, class: &str) -> String {
    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;background:#fff;color:#000}}\
         p{{width:120px;font:16px/20px sans-serif;margin:0}} {css}</style>\
         </head><body>{}</body></html>",
        body.replace("CLASS", class)
    )
}

const BOX: &str = "#s{background:red;height:20px} #s:hover{background:green} #s.b{background:blue}";
const DIV: &str = "<div id=s class='CLASS'></div>";

/// Whether the edit to `class` with the pointer at `pointer` kept the frame,
/// the frame and a fresh document's under the same pointer.
fn edit(
    cx: &mut TestAppContext,
    (css, body): (&str, &str),
    class: &str,
    pointer: Point<gpui::Pixels>,
) -> (bool, Vec<String>, Vec<String>) {
    let at = |class: &str| page(css, body, class);
    let (view, vcx) = settled(cx, &at(""), &at("x"));
    vcx.simulate_mouse_move(pointer, None, Modifiers::default());
    update(&view, vcx, &at(""));
    update(&view, vcx, &at(""));
    let before = frames(&view, vcx);
    update(&view, vcx, &at(class));
    let kept = frames(&view, vcx) == (before.0, before.1 + 1);
    let edited = quads(vcx);
    let (_, fresh) = open(cx, &at(class));
    fresh.simulate_mouse_move(pointer, None, Modifiers::default());
    frame(fresh);
    (kept, edited, quads(fresh))
}

#[gpui::test]
fn a_hover_box_away_from_the_pointer_is_recorded_again(cx: &mut TestAppContext) {
    let (kept, edited, fresh) = edit(cx, (BOX, DIV), "b", point(px(10.), px(300.)));
    assert_eq!(edited, fresh, "the kept frame differs from a fresh one");
    assert!(kept, "the frame was built");
}

#[gpui::test]
fn a_hovered_box_is_recorded_again_with_its_hover(cx: &mut TestAppContext) {
    let (kept, edited, fresh) = edit(cx, (BOX, DIV), "b", point(px(10.), px(10.)));
    assert_eq!(edited, fresh, "the kept frame differs from a fresh one");
    assert!(kept, "the frame was built");
}

#[gpui::test]
fn an_inline_box_with_a_hover_style_takes_its_band(cx: &mut TestAppContext) {
    let css = "a:hover{color:red} a.b{background:#0af}";
    let body = "<p>one <a id=s class='CLASS'>two three</a> end</p>";
    let (kept, edited, fresh) = edit(cx, (css, body), "b", point(px(10.), px(300.)));
    assert_eq!(edited, fresh, "the kept frame differs from a fresh one");
    assert!(kept, "the frame was built");
}

/// The pointer that enters, then leaves, the hover box of a settled frame:
/// each scene equals a fresh one's under the same pointer. A dispatched move
/// still counts as an input that may change element state
/// (`gpui::Window::interaction_epoch`), so this frame is built; a replay
/// records the boxes whose hover changed (`layer_hover_changes`).
#[gpui::test]
fn the_pointer_entering_and_leaving_a_hover_box_shows_its_hover(cx: &mut TestAppContext) {
    let html = page(BOX, DIV, "");
    let steps = [
        ("enters", point(px(10.), px(10.))),
        ("leaves", point(px(10.), px(300.))),
    ];
    let fresh: Vec<_> = steps
        .iter()
        .map(|(_, pointer)| {
            let (_, fresh) = open(cx, &html);
            fresh.simulate_mouse_move(*pointer, None, Modifiers::default());
            frame(fresh);
            quads(fresh)
        })
        .collect();
    let (view, vcx) = settled(cx, &html, &page(BOX, DIV, "x"));
    update(&view, vcx, &html);
    update(&view, vcx, &html);
    for ((step, pointer), fresh) in steps.into_iter().zip(fresh) {
        vcx.simulate_mouse_move(pointer, None, Modifiers::default());
        frame(vcx);
        assert_eq!(
            quads(vcx),
            fresh,
            "the pointer {step}: the frame differs from a fresh one"
        );
    }
}

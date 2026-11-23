//! Deferred paint owners retain their identity without changing z-order.
use super::box_repaint_tests::{frames, fresh, quads};
use super::reuse_tests::{settled, update};
use gpui::TestAppContext;

fn page(class: &str) -> String {
    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;background:white}}        #a,#b{{position:fixed;left:0;top:0;width:32px;height:32px;background:red}}        #a{{z-index:2147483647}} #b{{z-index:10;left:16px;background:blue}}        .on #a{{background:green}} .on #b{{background:yellow}}        .empty #a{{background:none}} .wide #a{{width:48px}}        .on p{{background:purple}} p{{height:80px;background:gray;margin:0}}        </style></head><body class='{class}'><p>flow</p><div id=b></div><div id=a></div></body></html>"
    )
}

#[gpui::test]
fn lifted_fills_change_appear_and_disappear_without_building(cx: &mut TestAppContext) {
    let steps = ["on", "empty", "on", ""];
    let expected: Vec<_> = steps.iter().map(|s| fresh(cx, &page(s))).collect();
    let (view, vcx) = settled(cx, &page(""), &page("x"));
    update(&view, vcx, &page(""));
    update(&view, vcx, &page(""));
    let original = quads(vcx);
    for (class, expected) in steps.into_iter().zip(expected) {
        let before = frames(&view, vcx);
        update(&view, vcx, &page(class));
        let scene = quads(vcx);
        assert_eq!(frames(&view, vcx), (before.0, before.1 + 1), "{class}");
        assert_eq!(scene, expected, "fresh {class}");
        if class == "on" {
            assert_ne!(scene, original, "the edit must be visible");
        }
    }
}

#[gpui::test]
fn lifted_geometry_changes_match_fresh_then_paint_edits_replay(cx: &mut TestAppContext) {
    let wide = fresh(cx, &page("wide"));
    let on = fresh(cx, &page("wide on"));
    let (view, vcx) = settled(cx, &page(""), &page("x"));
    update(&view, vcx, &page(""));
    update(&view, vcx, &page(""));
    update(&view, vcx, &page("wide"));
    assert_eq!(quads(vcx), wide);
    update(&view, vcx, &page("wide"));
    update(&view, vcx, &page("wide"));
    let before = frames(&view, vcx);
    update(&view, vcx, &page("wide on"));
    assert_eq!(frames(&view, vcx), (before.0, before.1 + 1));
    assert_eq!(quads(vcx), on);
}

#[gpui::test]
fn an_initially_empty_lifted_box_can_gain_ink(cx: &mut TestAppContext) {
    let expected = fresh(cx, &page("on"));
    let (view, vcx) = settled(cx, &page("empty"), &page("empty x"));
    update(&view, vcx, &page("empty"));
    update(&view, vcx, &page("empty"));
    update(&view, vcx, &page("on"));
    assert_eq!(quads(vcx), expected);
}

/// Root and nested scroll paths must retain the fixed and positive-z paint.
#[gpui::test]
fn lifted_edits_after_scroll_match_a_fresh_document(cx: &mut TestAppContext) {
    use super::tests::{frame, open};
    use gpui::{ScrollDelta, ScrollWheelEvent, point, px};
    for nested in [false, true] {
        let page = |class: &str| {
            let html = page(class).replace("height:80px", "height:1200px");
            let html = html.replace("#b{z-index:10", "#b{position:absolute;z-index:10");
            let html = html
                .replace(".on #b", ".unused #b")
                .replace(".on p", ".unused p");
            if nested {
                html.replace(
                    "<p>flow",
                    "<section style='position:relative;height:240px;overflow:auto'><p>flow",
                )
                .replace("</body>", "</section></body>")
            } else {
                html
            }
        };
        let scroll = |vcx: &mut gpui::VisualTestContext| {
            vcx.simulate_event(ScrollWheelEvent {
                position: point(px(100.), px(100.)),
                delta: ScrollDelta::Pixels(point(px(0.), px(-120.))),
                ..Default::default()
            });
            frame(vcx);
        };
        let (view, vcx) = settled(cx, &page(""), &page("x"));
        update(&view, vcx, &page(""));
        update(&view, vcx, &page(""));
        scroll(vcx);
        let before = frames(&view, vcx);
        update(&view, vcx, &page("on"));
        // Nested flows retain their existing conservative rebuild path.
        if !nested {
            assert_eq!(frames(&view, vcx), (before.0, before.1 + 1));
        }
        let edited = quads(vcx);
        let (_, fresh) = open(cx, &page("on"));
        frame(fresh);
        scroll(fresh);
        assert_eq!(edited, quads(fresh), "nested={nested}");
    }
}

//! Replaced content keeps a CSS paint owner even before its first decoration.

use super::box_repaint_tests::frames;
use super::reuse_tests::{settled, update};
use super::svg_paint_tests::scene;
use super::tests::{frame, open};
use gpui::{TestAppContext, px, size};

fn page(class: &str, display: &str, below: bool) -> String {
    let spacer = if below {
        "<div style='height:2400px'></div>"
    } else {
        ""
    };
    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;background:white}}\
         svg{{display:{display};vertical-align:top}} .a{{background:#abc}}\
         .b{{background:#def}}</style></head><body>{spacer}\
         before<svg class='{class}' width=80 height=60>\
         <rect x=5 y=5 width=65 height=45 fill='#397'/></svg>after</body></html>"
    )
}

fn edits(cx: &mut TestAppContext, display: &str, below: bool) {
    let html = |class: &str| page(class, display, below);
    let (view, vcx) = settled(cx, &html(""), &html("x"));
    vcx.simulate_resize(size(px(400.), px(300.)));
    frame(vcx);
    update(&view, vcx, &html(""));
    update(&view, vcx, &html(""));
    let mut scenes = Vec::new();
    for class in ["a", "b", ""] {
        let before = frames(&view, vcx);
        update(&view, vcx, &html(class));
        assert_eq!(
            frames(&view, vcx),
            (before.0, before.1 + 1),
            "{display}, below={below}, class={class}: rebuilt the frame"
        );
        scenes.push(scene(vcx));
    }
    update(&view, vcx, &html("a"));
    vcx.simulate_resize(size(px(400.), px(4200.)));
    frame(vcx);
    let revealed = scene(vcx);
    for (class, edited) in ["a", "b", ""].into_iter().zip(scenes) {
        let (_, fresh) = open(cx, &html(class));
        fresh.simulate_resize(size(px(400.), px(300.)));
        frame(fresh);
        assert_eq!(edited, scene(fresh), "{display}, below={below}, {class}");
    }
    let (_, fresh) = open(cx, &html("a"));
    fresh.simulate_resize(size(px(400.), px(4200.)));
    frame(fresh);
    assert_eq!(revealed, scene(fresh), "revealed stale replacement paint");
}

#[gpui::test]
fn replaced_background_appears_changes_and_disappears_through_its_owner(cx: &mut TestAppContext) {
    for display in ["inline", "block"] {
        for below in [false, true] {
            edits(cx, display, below);
        }
    }
}

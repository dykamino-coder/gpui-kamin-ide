//! Table auto margins resolve to zero on overflow and absorb positive free
//! space (CSS 2 sections 10.3.3 and 17.5.2.2; Flexbox section 8.1).

use super::super::layered_replay_tests::scene;
use super::super::tests::{frame, open};
use super::{replayed, resize};
use gpui::{Modifiers, TestAppContext, point, px};

#[gpui::test]
fn table_auto_margins_follow_free_space_after_replaying_an_overflow(cx: &mut TestAppContext) {
    for margins in ["margin:0 auto", "margin-left:auto", "margin-right:auto"] {
        // Interior flow blocks are eligible for replay; reset the embedded
        // UA table margins so this fixture isolates horizontal auto margins.
        let html = format!(
            "<!doctype html><style>body{{margin:0}}table{{border-spacing:0;margin:0;{margins}}}\
             td{{padding:0}}td div{{width:500px;height:20px;background:blue}}</style>\
             <div></div><table><tr><td><div></div></td></tr></table><div></div>"
        );
        let mut app = cx.clone();
        let (view, vcx) = open(&mut app, &html);
        // Hover listeners under the default pointer at the origin prevent
        // recording; this test isolates width-dependent auto margins.
        vcx.simulate_mouse_move(point(px(395.), px(290.)), None, Modifiers::default());
        frame(vcx);
        resize(&view, vcx, 401.);
        let before = replayed(&view, vcx);
        resize(&view, vcx, 400.);
        assert_eq!(replayed(&view, vcx) - before, 1, "{margins}");
        for width in [600., 399., 800., 400.] {
            resize(&view, vcx, width);
            let actual = scene(vcx);
            let mut fresh_app = cx.clone();
            let (fresh, fresh_cx) = open(&mut fresh_app, &html);
            fresh_cx.update(|_, cx| fresh.update(cx, |v, _| v.viewport.0 = width));
            frame(fresh_cx);
            assert_eq!(actual, scene(fresh_cx), "{margins} at {width}");
        }
    }
}

#[gpui::test]
fn negative_table_end_margin_does_not_prove_zero_free_space(cx: &mut TestAppContext) {
    let html = "<!doctype html><style>body{margin:0}table{border-spacing:0;\
                margin-left:auto;margin-right:-100px}td{padding:0}\
                td div{width:500px;height:20px;background:blue}</style>\
                <table><tr><td><div></div></td></tr></table>";
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, html);
    frame(vcx);
    resize(&view, vcx, 401.);
    for width in [400., 399., 450., 400.] {
        resize(&view, vcx, width);
        let actual = scene(vcx);
        let mut fresh_app = cx.clone();
        let (fresh, fresh_cx) = open(&mut fresh_app, html);
        fresh_cx.update(|_, cx| fresh.update(cx, |v, _| v.viewport.0 = width));
        frame(fresh_cx);
        assert_eq!(actual, scene(fresh_cx), "negative end margin at {width}");
    }
}

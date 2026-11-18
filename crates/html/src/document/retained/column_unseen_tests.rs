//! An own-paint edit inside a fragmented source must preserve its geometry.

use super::box_repaint_tests::{first_difference, frames, quads};
use super::reuse_tests::{settled, update};
use super::tests::{View, frame, open};
use gpui::{TestAppContext, VisualTestContext, px, size};

fn page(edit: &str) -> String {
    let mut rows = String::new();
    for i in 0..160 {
        if i % 40 == 0 {
            rows.push_str("<h2>Spanning heading</h2>");
        }
        let mark = if i == 80 { edit } else { "" };
        rows.push_str(&format!(
            "<p class=item{mark}><span class=float></span>One two three four five six seven eight nine ten eleven twelve.</p>"
        ));
    }
    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;color:#000;background:#fff}}\
         body{{font:16px Arial}} .sheet{{width:780px;columns:4;column-gap:12px;column-fill:balance}}\
         .item{{margin:0 0 3px;break-inside:avoid;background:#def}}\
         h2{{column-span:all;font-size:18px;margin:2px}}\
         .float{{float:left;width:30px;height:40px;background:#a73;margin:2px}}\
         [data-edit]{{background:#eee}}[data-edit=again]{{background:#aef}}\
         [data-edit=clear]{{background:transparent}}\
         [data-edit=gradient]{{background:linear-gradient(red,blue)}}\
         [data-edit=mixed]{{background:#eee;opacity:.5}}\
         [data-edit=geometry]{{background:#eee;padding-top:4px}}</style></head><body><div class=sheet>{rows}</div></body></html>"
    )
}

fn grow(view: &gpui::Entity<View>, cx: &mut VisualTestContext) {
    cx.simulate_resize(size(px(800.), px(6000.)));
    cx.update(|_, cx| view.update(cx, |v, _| v.viewport.1 = 6000.));
    frame(cx);
}

#[gpui::test]
fn an_unseen_float_paragraph_keeps_the_fragmented_frame(cx: &mut TestAppContext) {
    let (view, vcx) = settled(cx, &page(""), &page(" data-x"));
    update(&view, vcx, &page(""));
    update(&view, vcx, &page(""));
    let before = frames(&view, vcx);
    update(&view, vcx, &page(" data-edit"));
    let kept = frames(&view, vcx) == (before.0, before.1 + 1);
    assert_eq!(
        vcx.update(|_, cx| view.read(cx).doc.reuse.deferred_repaints()),
        1,
        "the omitted owner must wait for a fresh recording"
    );
    let once = frames(&view, vcx);
    update(&view, vcx, &page(" data-edit=again"));
    let repeated = frames(&view, vcx) == (once.0, once.1 + 1);
    assert_eq!(
        vcx.update(|_, cx| view.read(cx).doc.reuse.deferred_repaints()),
        1,
        "repeated edits must share one deferred owner"
    );
    let scene = quads(vcx);
    grow(&view, vcx);
    let grown = quads(vcx);
    assert_eq!(
        vcx.update(|_, cx| view.read(cx).doc.reuse.deferred_repaints()),
        0,
        "a fresh recording must discharge the deferred owner"
    );
    let (fresh_view, fresh) = open(cx, &page(" data-edit=again"));
    frame(fresh);
    let expected = quads(fresh);
    grow(&fresh_view, fresh);
    assert_eq!(first_difference(&scene, &expected), None);
    assert_ne!(scene, grown, "the larger window must expose more paint");
    assert_eq!(first_difference(&grown, &quads(fresh)), None);
    let (old_view, old) = open(cx, &page(""));
    frame(old);
    grow(&old_view, old);
    assert_ne!(grown, quads(old), "the changed fill must be exposed");
    assert!(
        repeated,
        "a second deferred edit rebuilt the fragmented frame"
    );
    assert!(
        kept,
        "an unseen own-paint edit rebuilt the fragmented frame"
    );
}

#[gpui::test]
fn another_owners_edit_preserves_the_omitted_repaint(cx: &mut TestAppContext) {
    let (view, vcx) = settled(cx, &page(""), &page(" data-x"));
    update(&view, vcx, &page(""));
    update(&view, vcx, &page(""));
    update(&view, vcx, &page(" data-edit"));
    let changed =
        page(" data-edit").replacen("<p class=item>", "<p class=item data-edit=again>", 1);
    update(&view, vcx, &changed);
    assert_eq!(
        vcx.update(|_, cx| view.read(cx).doc.reuse.deferred_repaints()),
        1,
        "a visible owner's value write must preserve the omitted owner's debt"
    );
    let scene = quads(vcx);
    grow(&view, vcx);
    let grown = quads(vcx);
    assert_eq!(
        vcx.update(|_, cx| view.read(cx).doc.reuse.deferred_repaints()),
        0,
        "fresh recording must paint both owners and discharge the debt"
    );
    let (fresh_view, fresh) = open(cx, &changed);
    frame(fresh);
    assert_eq!(first_difference(&scene, &quads(fresh)), None);
    grow(&fresh_view, fresh);
    assert_eq!(first_difference(&grown, &quads(fresh)), None);
}

#[gpui::test]
fn an_omitted_fragment_with_nonvalue_damage_rebuilds_as_fresh(cx: &mut TestAppContext) {
    for kind in ["clear", "gradient", "mixed", "geometry"] {
        let changed = page(&format!(" data-edit={kind}"));
        let (view, vcx) = settled(cx, &page(""), &page(" data-x"));
        update(&view, vcx, &page(""));
        update(&view, vcx, &page(""));
        update(&view, vcx, &page(" data-edit"));
        let before = frames(&view, vcx);
        update(&view, vcx, &changed);
        let after = frames(&view, vcx);
        // Replacing the tree resets its counters; a replay keeps both.
        assert_ne!(
            after,
            (before.0, before.1 + 1),
            "{kind} must build a fresh frame"
        );
        assert_eq!(
            vcx.update(|_, cx| view.read(cx).doc.reuse.deferred_repaints()),
            0,
            "{kind} must discharge the old deferred record"
        );
        let scene = quads(vcx);
        grow(&view, vcx);
        let grown = quads(vcx);
        let (fresh_view, fresh) = open(cx, &changed);
        frame(fresh);
        assert_eq!(first_difference(&scene, &quads(fresh)), None, "{kind}");
        grow(&fresh_view, fresh);
        assert_eq!(first_difference(&grown, &quads(fresh)), None, "{kind}");
    }
}

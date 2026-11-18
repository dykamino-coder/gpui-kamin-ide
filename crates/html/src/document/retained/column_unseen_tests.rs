//! Paint-only edits of boxes in a multi-column container that runs far
//! below the screen. The commands of a column piece off the screen are cut
//! away by its clip where the piece's record is placed (`gpui`
//! `scene::layer::placed`); the layer still holds the boxes of the piece by
//! empty runs, as it holds a box painted directly whose commands the content
//! mask culled. An edit of such a box keeps the recorded frame, its record
//! waiting until a frame shows it; the frame now, after a taller window and
//! in a taller viewport equals a fresh document's.

use super::box_repaint_tests::{first_difference, frames, quads};
use super::reuse_tests::{settled, update};
use super::tests::{View, frame, open};
use gpui::{TestAppContext, VisualTestContext, px, size};

/// What the edit marks in section `at`.
#[derive(Clone, Copy)]
enum Target {
    /// The column spanner heading the section.
    Spanner,
    /// The third item of the section (its fill changes).
    Item,
    /// The float in the third item (a fill painted in another fill).
    Float,
}

/// Sections of a spanner and forty items, each with a float, in four
/// balanced columns (the shape of the `columns-N` bench pages). `edit` goes
/// on `target` of section `at`.
fn page(at: usize, target: Target, edit: &str) -> String {
    let sections: String = (0..10)
        .map(|s| {
            let mark = |t: Target, i: usize| {
                let on = s == at
                    && match t {
                        Target::Spanner => matches!(target, Target::Spanner),
                        Target::Item => matches!(target, Target::Item) && i == 2,
                        Target::Float => matches!(target, Target::Float) && i == 2,
                    };
                if on { edit } else { "" }
            };
            let items: String = (0..40)
                .map(|i| {
                    format!(
                        "<p class=i{}><span class=f{}></span>One two three four five six \
                         seven eight nine ten eleven twelve.</p>",
                        mark(Target::Item, i),
                        mark(Target::Float, i)
                    )
                })
                .collect();
            format!("<h2{}>Section</h2>{items}", mark(Target::Spanner, 0))
        })
        .collect();
    format!(
        "<!doctype html><html><head><style>html{{background:white}}\
         body{{margin:0;font:16px Arial;color:#152238}}*{{box-sizing:border-box}}\
         .m{{width:780px;columns:4;column-gap:12px;column-fill:balance}}\
         .i{{margin:0 0 3px;break-inside:avoid;background:#def}}\
         h2{{column-span:all;font-size:18px;margin:2px}}\
         .f{{float:left;width:30px;height:40px;background:#a73;margin:2px}}\
         [data-edit]{{background:#eee}}</style></head><body><div class=m>{sections}</div>\
         </body></html>"
    )
}

fn window(cx: &mut VisualTestContext, height: f32) {
    cx.simulate_resize(size(px(800.), px(height)));
    frame(cx);
}

/// A taller window, which shows every section.
fn grown(_: &gpui::Entity<View>, cx: &mut VisualTestContext) {
    window(cx, 9000.);
}

/// A taller viewport in the same window.
fn tall(view: &gpui::Entity<View>, cx: &mut VisualTestContext) {
    cx.update(|_, cx| view.update(cx, |v, _| v.viewport.1 = 9000.));
    frame(cx);
}

type Then = fn(&gpui::Entity<View>, &mut VisualTestContext);

/// The edit from a settled frame in a 600px window: whether it kept the
/// frame without a build, how many box records wait, the scene after it and
/// after `then`.
fn edited(
    cx: &mut TestAppContext,
    at: usize,
    target: Target,
    then: Then,
) -> (bool, usize, Vec<String>, Vec<String>) {
    let page = |edit: &str| page(at, target, edit);
    let (view, vcx) = settled(cx, &page(""), &page(" data-x"));
    window(vcx, 600.);
    update(&view, vcx, &page(" data-x"));
    update(&view, vcx, &page(""));
    update(&view, vcx, &page(""));
    let before = frames(&view, vcx);
    update(&view, vcx, &page(" data-edit"));
    let kept = frames(&view, vcx) == (before.0, before.1 + 1);
    let waiting = vcx.update(|_, cx| view.read(cx).doc.reuse.deferred_repaints());
    let now = quads(vcx);
    then(&view, vcx);
    (kept, waiting, now, quads(vcx))
}

/// A fresh document of the edited page in the 600px window, then `then`.
fn fresh(cx: &mut TestAppContext, at: usize, target: Target, then: Then) -> [Vec<String>; 2] {
    let (view, vcx) = open(cx, &page(at, target, " data-edit"));
    window(vcx, 600.);
    let now = quads(vcx);
    then(&view, vcx);
    [now, quads(vcx)]
}

/// The edit keeps the frame, waits (`defers`) or not, and every frame
/// equals a fresh document's.
fn check(cx: &mut TestAppContext, at: usize, target: Target, defers: bool, then: Then) {
    let (kept, waiting, now, after) = edited(cx, at, target, then);
    let [fresh_now, fresh_after] = fresh(cx, at, target, then);
    assert_eq!(first_difference(&now, &fresh_now), None, "the frame after the edit");
    assert_eq!(first_difference(&after, &fresh_after), None, "the frame showing it");
    assert!(kept, "the edit built the frame");
    assert_eq!(waiting > 0, defers, "records waiting: {waiting}");
}

#[gpui::test]
fn an_item_far_below_the_screen_keeps_the_frame(cx: &mut TestAppContext) {
    check(cx, 5, Target::Item, true, grown);
}

#[gpui::test]
fn an_item_far_below_the_screen_shows_as_fresh_in_a_taller_viewport(cx: &mut TestAppContext) {
    check(cx, 5, Target::Item, true, tall);
}

#[gpui::test]
fn a_float_far_below_the_screen_keeps_the_frame(cx: &mut TestAppContext) {
    check(cx, 5, Target::Float, true, grown);
}

#[gpui::test]
fn a_spanner_far_below_the_screen_keeps_the_frame(cx: &mut TestAppContext) {
    check(cx, 5, Target::Spanner, true, grown);
}

#[gpui::test]
fn an_item_on_the_screen_is_written_now(cx: &mut TestAppContext) {
    check(cx, 0, Target::Item, false, grown);
}

#[gpui::test]
fn a_spanner_on_the_screen_is_recorded_now(cx: &mut TestAppContext) {
    check(cx, 0, Target::Spanner, false, grown);
}

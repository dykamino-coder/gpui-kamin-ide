//! Own paint of a box changed by a restyle (`box_repaint`): a fill that
//! appears or goes and a new fill colour reach the
//! document's scene layer without a build of the frame, and the frame equals
//! a fresh document's; a box whose paint is not its own commands only
//! (outline, a decoration painted as a child element) builds the frame.

use super::reuse_tests::{settled, update};
use super::tests::{View, frame, open};
use gpui::{TestAppContext, VisualTestContext};

fn page(edit: &str) -> String {
    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;color:#000;background:#fff}}\
         .b{{height:20px;border:2px solid #000}} [data-edit]{{background:#eee}}\
         [data-edit=\"2\"]{{background:#ddd}} [data-edit=\"4\"]{{outline:3px solid #00f}}</style></head><body>\
         <div class=b>one</div><div class=b{edit}><p>two</p></div><div class=b>three</div>\
         </body></html>"
    )
}

/// The first quad where `a` and `b` differ, and their lengths.
pub(super) fn first_difference(
    a: &[String],
    b: &[String],
) -> Option<(usize, usize, usize, String, String)> {
    let at = (0..a.len().max(b.len())).find(|&i| a.get(i) != b.get(i))?;
    let get = |v: &[String]| v.get(at).cloned().unwrap_or_default();
    Some((at, a.len(), b.len(), get(a), get(b)))
}

pub(super) fn frames(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> (u64, u64) {
    cx.update(|_, cx| view.read(cx).doc.retained_frames())
}

/// The painted quads in paint order, without their draw orders: a box
/// recorded again keeps its key, a fresh frame numbers its own.
pub(super) fn quads(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|window, _| {
        window
            .painted_quads()
            .into_iter()
            .map(|mut q| {
                q.order = 0;
                format!("{q:?}")
                    .replace(" (scaled)", "")
                    .replace("Quad { order: 0, border_style: Solid, ", "")
            })
            .collect()
    })
}

pub(super) fn fresh(cx: &mut TestAppContext, html: &str) -> Vec<String> {
    let (_, fresh) = open(cx, html);
    frame(fresh);
    quads(fresh)
}

/// Builds and replays of the frame for each edit of the edited box, from a
/// settled frame of `page("")`, with the scene after each and a fresh one.
fn edits(cx: &mut TestAppContext, steps: &[&str]) -> Vec<(bool, Vec<String>, Vec<String>)> {
    let (view, vcx) = settled(cx, &page(""), &page(" data-x"));
    update(&view, vcx, &page(""));
    update(&view, vcx, &page(""));
    let mut out = Vec::new();
    let mut scenes = Vec::new();
    for step in steps {
        let before = frames(&view, vcx);
        update(&view, vcx, &page(step));
        let after = frames(&view, vcx);
        // Replayed: the same slot, one replay more and no build.
        out.push(after == (before.0, before.1 + 1));
        scenes.push(quads(vcx));
    }
    let fresh: Vec<_> = steps.iter().map(|s| fresh(cx, &page(s))).collect();
    out.into_iter()
        .zip(scenes)
        .zip(fresh)
        .map(|((n, s), f)| (n, s, f))
        .collect()
}

#[gpui::test]
fn a_fill_that_appears_changes_colour_and_goes_is_recorded_without_a_build(
    cx: &mut TestAppContext,
) {
    let steps = [" data-edit", " data-edit=\"2\"", ""];
    for (i, (counts, scene, fresh)) in edits(cx, &steps).into_iter().enumerate() {
        assert!(counts, "edit {:?} built the frame", steps[i]);
        assert_eq!(
            first_difference(&scene, &fresh),
            None,
            "edit {:?} differs from a fresh frame",
            steps[i]
        );
    }
}

#[gpui::test]
fn an_outline_builds_the_frame(cx: &mut TestAppContext) {
    let steps = [" data-edit=\"4\""];
    for (counts, scene, fresh) in edits(cx, &steps) {
        assert!(!counts, "an outline paints as an element of the frame");
        assert_eq!(first_difference(&scene, &fresh), None);
    }
}

#[gpui::test]
fn an_inactive_hover_painter_keeps_the_recorded_box(cx: &mut TestAppContext) {
    let (kept, edited, fresh) = super::band_rerecord_tests::edit(
        cx,
        "#s{pointer-events:none;background:red;height:20px} #s:hover{background:green} #s.b{background:blue}",
        "<div id=s class='CLASS'></div>",
        "b",
    );
    assert_eq!(edited, fresh);
    assert!(kept);
}

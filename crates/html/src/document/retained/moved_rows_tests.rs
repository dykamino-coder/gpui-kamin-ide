//! Edits by mutation records (`Document::apply`) that move the root-flow
//! blocks below them: the rows replay their output at the new place.

use super::content_replay_tests::{cx_replays, placed};
use super::tests::{View, frame, open};
use gpui::{TestAppContext, VisualTestContext};

/// 600 rows; the class `class` on row 3 sets its top padding.
// Exact line heights isolate integer movement from accumulated f32 phases.
fn padded(class: &str) -> String {
    let rows: String = (0..600)
        .map(|i| {
            let c = if i == 3 { class } else { "" };
            format!("<div class='r {c}'>row {i} <b>x</b></div>")
        })
        .collect();
    format!(
        "<!doctype html><html><head><style>body{{margin:0}} \
         .r{{line-height:20px;border-bottom:1px solid #abc}} .big{{padding-top:40px}}</style></head>\
         <body>{rows}</body></html>"
    )
}

/// Apply `html` to the view's document as mutation records, as a script would.
fn apply(view: &gpui::Entity<View>, cx: &mut VisualTestContext, html: &str) {
    cx.update(|_, cx| {
        view.update(cx, |v, _| {
            let records = v.doc.mutations(html).expect("same shape");
            v.doc.apply(&records, "").unwrap();
        })
    });
    frame(cx);
}

/// A script grows and shrinks one row near the top: the rows below it are
/// not built again, those on the screen replay their output at their new
/// place, and only the restyled row and the rows the shrink brings onto the
/// screen are built.
#[gpui::test]
fn rows_below_a_restyled_row_replay_moved(cx: &mut TestAppContext) {
    let (view, vcx) = open(cx, &padded(""));
    frame(vcx);
    frame(vcx);
    for class in ["big", "", "big", ""] {
        apply(&view, vcx, &padded(class));
    }
    let mut frames = Vec::new();
    for class in ["big", ""] {
        let replays = cx_replays(&view, vcx);
        let (recorded, skipped, _) = super::reuse_tests::reused(&view, vcx);
        apply(&view, vcx, &padded(class));
        let (now, kept, _) = super::reuse_tests::reused(&view, vcx);
        let replayed = cx_replays(&view, vcx) - replays;
        assert!(
            replayed >= 40,
            "'{class}': rows on screen replayed {replayed}"
        );
        assert!(
            kept - skipped > 500,
            "'{class}': rows kept {}",
            kept - skipped
        );
        assert!(
            now - recorded < 100,
            "'{class}': rows built {}",
            now - recorded
        );
        frames.push((class, placed(vcx)));
    }
    for (class, after) in frames {
        let (_, fresh) = open(cx, &padded(class));
        frame(fresh);
        assert_eq!(
            after,
            placed(fresh),
            "'{class}': the frame differs from a fresh one"
        );
    }
}

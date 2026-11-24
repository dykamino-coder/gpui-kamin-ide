//! A class edit whose restyle changes layout values keeps the box tree
//! (`revision::repaint::relayout_only`): the next frame lays the restyled box
//! out again, the clean blocks around it replay, and the frame is the one a
//! fresh document draws.

use super::reuse_tests::{settled, update};
use super::tests::{frame, open};
use gpui::{Bounds, TestAppContext, VisualTestContext, point, px, size};

fn page(class: &str) -> String {
    let rows: String = (0..24)
        .map(|i| format!("<p style='height:20px;margin:0;background:#{:03x}'>r{i}</p>", i * 97 % 4096))
        .collect();
    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;background:white}}\
         #a{{background:#08f;height:10px}} #a.s8{{padding-top:8px}} #a.s75{{padding-top:7.5px}}</style></head>\
         <body><div id=a class='{class}'></div>{rows}</body></html>"
    )
}

#[gpui::test]
fn a_geometry_class_edit_relayouts_without_a_rebuild(cx: &mut TestAppContext) {
    for shift in ["s8", "s75"] {
        let expected = fresh(cx, &page(shift));
        let back = fresh(cx, &page(""));
        let (view, vcx) = settled(cx, &page(""), &page("x"));
        update(&view, vcx, &page(""));
        update(&view, vcx, &page(""));
        let replayed = vcx.update(|_, cx| view.read(cx).doc.replayed_blocks());
        update(&view, vcx, &page(shift));
        same(&quads(vcx), &expected, &format!("shift {shift}"));
        update(&view, vcx, &page(""));
        same(&quads(vcx), &back, &format!("back from {shift}"));
        let after = vcx.update(|_, cx| view.read(cx).doc.replayed_blocks());
        assert!(after > replayed, "shift {shift}: no block replayed around the edit");
    }
}

/// The painted quads equal, or the first that differs.
fn same(got: &[String], want: &[String], what: &str) {
    let at = got.iter().zip(want).position(|(a, b)| a != b);
    assert!(
        at.is_none() && got.len() == want.len(),
        "{what}: {} vs {} quads, first difference {at:?}:\n{:?}\n{:?}",
        got.len(),
        want.len(),
        at.map(|i| &got[i]),
        at.map(|i| &want[i]),
    );
}

/// The painted quads as pixels see them: a pure translation folded into the
/// bounds and the content mask cut to the window (a replay moves a quad by
/// its transformation and its mask with it, a fresh paint by its bounds).
fn quads(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|window, _| {
        let screen = Bounds::new(point(px(0.), px(0.)), size(px(800.), px(600.))).scale(1.0);
        window
            .painted_quads()
            .into_iter()
            .map(|mut q| {
                q.order = 0;
                let t = &mut q.transformation;
                if t.rotation_scale == [[1., 0.], [0., 1.]] {
                    q.bounds.origin.x.0 += t.translation[0];
                    q.bounds.origin.y.0 += t.translation[1];
                    t.translation = [0., 0.];
                }
                q.content_mask.bounds = q.content_mask.bounds.intersect(&screen);
                format!("{q:?}")
            })
            .collect()
    })
}

fn fresh(cx: &mut gpui::TestAppContext, html: &str) -> Vec<String> {
    let (_, fresh) = open(cx, html);
    frame(fresh);
    quads(fresh)
}
//! `position: fixed` against the root scrollport of the document view: a box
//! whose containing block is the viewport stays put while the root scrolls
//! (`layout::positioned::viewport_anchor`), a box under a fixed containing
//! block scrolls with it, and neither adds work to a scroll frame.

use super::tests::{frame, open};
use super::*;
use gpui::{
    Entity, Modifiers, ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext, point,
};

pub(super) const RED: &str = "width:50px;height:50px;background:red";

pub(super) fn rows(extra: &str) -> String {
    let rows: String = (0..400)
        .map(|i| format!("<div class=r>row {i}</div>"))
        .collect();
    format!(
        "<style>body{{margin:0}}.r{{height:20px}}.r:nth-child(odd){{background:#eee}}</style>\
         {extra}<main>{rows}</main>"
    )
}

pub(super) fn fixed_box() -> String {
    format!("<div style='position:fixed;top:100px;left:0;{RED}'></div>")
}

pub(super) fn wheel(cx: &mut VisualTestContext) {
    cx.simulate_event(ScrollWheelEvent {
        position: point(px(200.), px(20.)),
        delta: ScrollDelta::Pixels(point(px(0.), px(-120.))),
        modifiers: Modifiers::default(),
        touch_phase: gpui::TouchPhase::Moved,
    });
}

pub(super) fn root(view: &Entity<DocumentView>, cx: &mut VisualTestContext) -> gpui::ScrollHandle {
    cx.update(|_, cx| {
        let state = view.read(cx).document.state_rc();
        state.root_scroll.as_ref().unwrap().handle.clone()
    })
}

/// Origins (CSS px) of the painted 50x50 quads.
pub(super) fn boxes(cx: &mut VisualTestContext) -> Vec<(f32, f32)> {
    cx.update(|window, _| {
        let s = window.scale_factor();
        window
            .painted_quads()
            .into_iter()
            .filter(|q| {
                (q.bounds.size.width.0 / s - 50.).abs() < 0.5
                    && (q.bounds.size.height.0 / s - 50.).abs() < 0.5
            })
            // Placed: a moved replay translates the quad.
            .map(|q| {
                let t = q.transformation.translation;
                ((q.bounds.origin.x.0 + t[0]) / s, (q.bounds.origin.y.0 + t[1]) / s)
            })
            .collect()
    })
}

/// The painted quads in paint order, without their draw orders; a pure
/// translation is folded into the bounds (a scrolled replay moves a quad by
/// its transformation, a fresh paint by its bounds: the same pixels).
pub(super) fn quads(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|window, _| {
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
                format!("{q:?}")
            })
            .collect()
    })
}

/// The painted quads with their draw orders and drawn areas (`quads`).
pub(super) fn painted(cx: &mut VisualTestContext) -> Vec<(String, u32, gpui::Bounds<gpui::ScaledPixels>)> {
    let keys = quads(cx);
    let drawn: Vec<_> = cx.update(|window, _| {
        window
            .painted_quads()
            .into_iter()
            .map(|q| {
                let t = q.transformation.translation;
                let mut b = q.bounds;
                b.origin.x.0 += t[0];
                b.origin.y.0 += t[1];
                (q.order, b.intersect(&q.content_mask.bounds))
            })
            .collect()
    });
    keys.into_iter().zip(drawn).map(|(key, (order, area))| (key, order, area)).collect()
}

/// Whether two frames draw the same pixels: the same quads, and every two
/// that overlap in the same order. Orders of disjoint quads are free (the
/// bounds tree orders a quad only above what it overlaps).
pub(super) fn same_paint(
    a: &[(String, u32, gpui::Bounds<gpui::ScaledPixels>)],
    b: &[(String, u32, gpui::Bounds<gpui::ScaledPixels>)],
) -> Result<(), String> {
    fn sorted(list: &[(String, u32, gpui::Bounds<gpui::ScaledPixels>)]) -> Vec<&String> {
        let mut keys: Vec<&String> = list.iter().map(|q| &q.0).collect();
        keys.sort();
        keys
    }
    let (ka, kb) = (sorted(a), sorted(b));
    if ka != kb {
        let at = (0..ka.len().max(kb.len())).find(|&i| ka.get(i) != kb.get(i));
        return Err(format!(
            "quads differ at sorted {at:?} of {}/{}: {:?} vs {:?}",
            ka.len(),
            kb.len(),
            at.and_then(|i| ka.get(i)),
            at.and_then(|i| kb.get(i))
        ));
    }
    let find = |list: &[(String, u32, gpui::Bounds<gpui::ScaledPixels>)], key: &String| {
        list.iter().find(|q| &q.0 == key).map(|q| q.1)
    };
    for (i, x) in b.iter().enumerate() {
        for y in &b[i + 1..] {
            if x.0 == y.0 || x.2.intersect(&y.2).is_empty() {
                continue;
            }
            let (ox, oy) = (find(a, &x.0), find(a, &y.0));
            if (x.1 < y.1) != (ox < oy) {
                return Err(format!("overlapping quads swap order: {} / {}", x.0, y.0));
            }
        }
    }
    Ok(())
}

/// Frame builds and replays of the document so far.
fn frames(view: &Entity<DocumentView>, cx: &mut VisualTestContext) -> (u64, u64) {
    cx.update(|_, cx| view.read(cx).document.retained_frames())
}

#[gpui::test]
fn a_viewport_fixed_box_stays_while_the_root_scrolls(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, &rows(&fixed_box()));
    assert_eq!(boxes(cx), vec![(0., 100.)]);
    wheel(cx);
    frame(cx);
    assert!(root(&view, cx).offset().y < px(0.));
    assert_eq!(boxes(cx), vec![(0., 100.)], "wheel");
    cx.simulate_keystrokes("end");
    frame(cx);
    assert!(root(&view, cx).offset().y < px(-1000.));
    assert_eq!(boxes(cx), vec![(0., 100.)], "end");
    cx.simulate_keystrokes("home pagedown");
    frame(cx);
    assert_eq!(boxes(cx), vec![(0., 100.)], "pagedown");
}

#[gpui::test]
fn a_fixed_box_under_a_transform_scrolls_with_its_containing_block(cx: &mut TestAppContext) {
    let extra = format!(
        "<div style='transform:translateX(0)'><div style='position:fixed;top:100px;left:0;{RED}'></div></div>"
    );
    let (view, cx) = open(cx, &rows(&extra));
    assert_eq!(boxes(cx), vec![(0., 100.)]);
    wheel(cx);
    frame(cx);
    let dy = f32::from(root(&view, cx).offset().y);
    assert!(dy < 0.);
    assert_eq!(boxes(cx), vec![(0., 100. + dy)]);
}

#[gpui::test]
fn every_fixed_containing_block_of_the_standard_cancels_the_anchor(cx: &mut TestAppContext) {
    // css-transforms-1/2, filter-effects-1/2, css-contain-2, css-will-change-1.
    for cb in [
        "transform:translateX(0)",
        "perspective:100px",
        "filter:blur(0)",
        "backdrop-filter:blur(0)",
        "contain:paint",
        "contain:layout",
        "contain:strict",
        "contain:content",
        "will-change:transform",
    ] {
        let extra = format!(
            "<div style='{cb}'><div style='position:fixed;top:100px;left:0;{RED}'></div></div>"
        );
        let mut app = cx.clone();
        let (view, vcx) = open(&mut app, &rows(&extra));
        wheel(vcx);
        frame(vcx);
        let dy = f32::from(root(&view, vcx).offset().y);
        assert!(dy < 0., "{cb}: the root did not scroll");
        let at = boxes(vcx);
        assert!(
            at.iter().all(|&(_, y)| (y - (100. + dy)).abs() < 0.5),
            "{cb}: a fixed box with a fixed containing block stood still: {at:?}"
        );
    }
}

#[gpui::test]
fn an_absolute_box_of_the_root_scrolls_with_the_content(cx: &mut TestAppContext) {
    let extra = format!("<div style='position:absolute;top:100px;left:0;{RED}'></div>");
    let (view, cx) = open(cx, &rows(&extra));
    wheel(cx);
    frame(cx);
    let dy = f32::from(root(&view, cx).offset().y);
    assert_eq!(boxes(cx), vec![(0., 100. + dy)]);
}

#[gpui::test]
fn a_fixed_box_outside_the_viewport_adds_no_scroll_range(cx: &mut TestAppContext) {
    let html = format!(
        "<style>body{{margin:0}}</style><p>short</p>\
         <div style='position:fixed;top:2000px;left:3000px;{RED}'></div>"
    );
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &html);
    let max = root(&view, vcx).max_offset();
    assert_eq!((max.x, max.y), (px(0.), px(0.)));
    // An absolute box there extends the scrollable overflow (css-overflow-3).
    let html = html.replace("position:fixed", "position:absolute");
    let (view, vcx) = open(cx, &html);
    let max = root(&view, vcx).max_offset();
    assert!(max.x > px(0.) && max.y > px(0.));
}

#[gpui::test]
fn a_scrolled_frame_with_a_fixed_box_matches_a_fresh_one(cx: &mut TestAppContext) {
    matches_fresh(cx, &rows(&fixed_box()));
}

/// A frame scrolled by the wheel paints the quads of a new tree built at
/// that offset.
pub(super) fn matches_fresh(cx: &mut TestAppContext, html: &str) {
    let mut app = cx.clone();
    let (view, scrolled) = open(&mut app, html);
    wheel(scrolled);
    frame(scrolled);
    frame(scrolled);
    let offset = root(&view, scrolled).offset();
    let after = painted(scrolled);
    // The fresh frame: a new tree built at that offset, not a replay.
    let (fresh_view, fresh) = open(cx, html);
    root(&fresh_view, fresh).set_offset(offset);
    fresh.update(|_, cx| {
        fresh_view.update(cx, |view, cx| {
            view.document
                .update(&format!("{html}<!-- fresh -->"), "")
                .unwrap();
            cx.notify();
        })
    });
    frame(fresh);
    frame(fresh);
    assert_eq!(root(&fresh_view, fresh).offset(), offset);
    let fresh = painted(fresh);
    if let Err(diff) = same_paint(&after, &fresh) {
        panic!("scrolled and fresh frames differ: {diff}");
    }
}

#[gpui::test]
fn a_fixed_box_adds_no_frame_work_to_a_scroll(cx: &mut TestAppContext) {
    let mut work = Vec::new();
    for extra in [String::new(), fixed_box()] {
        let mut app = cx.clone();
        let (view, vcx) = open(&mut app, &rows(&extra));
        frame(vcx);
        let idle = frames(&view, vcx);
        frame(vcx);
        let replayed = frames(&view, vcx);
        // A settled frame replays whole, with or without the fixed box.
        assert_eq!(replayed, (idle.0, idle.1 + 1), "{extra}");
        wheel(vcx);
        frame(vcx);
        frame(vcx);
        let scrolled = frames(&view, vcx);
        work.push((scrolled.0 - replayed.0, scrolled.1 - replayed.1));
    }
    assert_eq!(
        work[0], work[1],
        "builds and replays of a scroll: plain, fixed"
    );
}

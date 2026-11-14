//! BR-29B: keyboard focus scrolls the production session list to its row.

use super::reveal::revealed_offset;
use super::*;
use crate::host_link::ShellEvent;
use gpui::{
    Bounds, Context, IntoElement, Render, ScrollHandle, TestAppContext, VisualTestContext, point,
    size,
};
use kamin_model::SessionsSnapshot;
use smol::channel::Sender;
use std::collections::HashSet;

const ROWS: usize = 30;

fn b(top: f32, height: f32) -> Bounds<gpui::Pixels> {
    Bounds::new(point(px(0.), px(top)), size(px(100.), px(height)))
}

#[test]
fn offset_moves_only_as_far_as_needed_and_stays_in_range() {
    let view = b(100., 200.);
    let max = point(px(0.), px(500.));
    let at = |y: f32| point(px(0.), px(y));
    // Below the viewport: the bottom edge meets the viewport bottom.
    assert_eq!(
        revealed_offset(view, b(310., 24.), at(0.), max),
        Some(at(-34.))
    );
    // Above the viewport: the top edge meets the viewport top.
    assert_eq!(
        revealed_offset(view, b(60., 24.), at(-200.), max),
        Some(at(-160.))
    );
    // Already visible: no change.
    assert_eq!(revealed_offset(view, b(150., 24.), at(-40.), max), None);
    // Taller than the viewport: align the top.
    assert_eq!(
        revealed_offset(view, b(400., 300.), at(0.), max),
        Some(at(-300.))
    );
    // Never past either end.
    assert_eq!(
        revealed_offset(view, b(900., 24.), at(0.), max),
        Some(at(-500.))
    );
    assert_eq!(
        revealed_offset(view, b(90., 24.), at(5.), max),
        Some(at(0.))
    );
}

struct Sidebar {
    snap: SessionsSnapshot,
    scroll: ScrollHandle,
    tx: Sender<ShellEvent>,
}

impl Render for Sidebar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        begin_frame(window);
        if flush(cx) {
            cx.notify();
        }
        let p = kamin_theme::ThemeKind::Dark.palette();
        div()
            .w(px(240.))
            .h(px(220.))
            .flex()
            .child(crate::ui::sessions_list::sessions_sidebar(
                Some(&self.snap),
                &HashSet::new(),
                None,
                None,
                &self.scroll,
                &self.tx,
                p,
            ))
    }
}

fn snapshot() -> SessionsSnapshot {
    let sessions: Vec<_> = (0..ROWS)
        .map(|i| {
            serde_json::json!({
                "id": format!("row-{i}"), "name": format!("Session {i}"),
                "projectId": "p", "open": true, "lastOpened": 0, "createdAt": i
            })
        })
        .collect();
    serde_json::from_value(serde_json::json!({
        "projects": [{"id": "p", "folderPath": null, "createdAt": 0}],
        "sessions": sessions,
        "activeSessionId": null
    }))
    .unwrap()
}

fn draw(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
}

/// The painted ring of the focused row lies fully inside the list viewport.
/// (Rounded rings are painted as several masked segments of one quad, so the
/// side-by-side clip check lives in `paint_tests`.)
fn ring_visible(scroll: &ScrollHandle, cx: &mut VisualTestContext) -> bool {
    cx.update(|window, _| {
        let accent = crate::colors::rgba(kamin_theme::ThemeKind::Dark.palette().accent_primary);
        let view = scroll.bounds().scale(window.scale_factor());

        window.painted_quads().into_iter().any(|q| {
            q.border_color == accent.into()
                && q.border_widths.top > px(0.).scale(1.)
                && q.bounds.intersect(&view) == q.bounds
        })
    })
}

#[gpui::test]
fn keyboard_focus_scrolls_the_session_list_to_the_row(cx: &mut TestAppContext) {
    let _guard = keymap_tests::SERIAL
        .lock()
        .unwrap_or_else(|err| err.into_inner());
    HANDLES.lock().unwrap().clear();
    PENDING.lock().unwrap().clear();
    *FRAME.lock().unwrap() = (false, None);
    cx.update(crate::startup::configure);
    let (tx, _rx) = smol::channel::unbounded();
    let scroll = ScrollHandle::new();
    let (_, cx) = cx.add_window_view(|_, _| Sidebar {
        snap: snapshot(),
        scroll: scroll.clone(),
        tx,
    });
    draw(cx);
    assert!(scroll.max_offset().y > px(0.), "the list must overflow");
    // Keyboard modality, as after Tab.
    cx.update(|window, cx| window.focus_next(cx));
    let last = format!("srow:row-{}", ROWS - 1);
    assert!(cx.update(|window, cx| focus_id(&last, window, cx)));
    draw(cx);
    assert!(scroll.offset().y < px(0.), "the list scrolled down");
    assert!(ring_visible(&scroll, cx), "the last row is visible");

    // Back to the first row, which is now above the viewport.
    let bottom = scroll.offset().y;
    assert!(cx.update(|window, cx| focus_id("srow:row-0", window, cx)));
    draw(cx);
    // Only as far as the row: the group header above it may stay hidden.
    assert!(scroll.offset().y > bottom, "the list scrolled up");
    assert!(ring_visible(&scroll, cx), "the first row is visible");

    // A later manual scroll is kept while the focus stays on the row.
    scroll.set_offset(point(px(0.), -scroll.max_offset().y));
    draw(cx);
    assert_eq!(scroll.offset().y, -scroll.max_offset().y);
}

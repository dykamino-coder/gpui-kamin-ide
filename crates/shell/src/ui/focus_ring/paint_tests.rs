//! BR-29A: настоящие строки и чипы сохраняют все стороны кольца внутри клипа.

use super::*;
use crate::host_link::ShellEvent;
use gpui::{Context, IntoElement, Render, TestAppContext, VisualTestContext, point};
use kamin_model::Session;
use smol::channel::Sender;

const ITEMS: [&str; 3] = ["first", "middle", "last"];

struct ClippedSessions {
    chips: bool,
    tx: Sender<ShellEvent>,
}

impl Render for ClippedSessions {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        begin_frame(window);
        if flush(cx) {
            cx.notify();
        }
        let p = kamin_theme::ThemeKind::Dark.palette();
        let mut clip = div().flex().overflow_hidden();
        clip = if self.chips {
            clip.w(px(381.)).h(px(28.))
        } else {
            clip.flex_col().w(px(240.)).h(px(72.))
        };
        for (index, id) in ITEMS.into_iter().enumerate() {
            let session: Session = serde_json::from_value(serde_json::json!({
                "id": id, "name": id, "projectId": "paint-fixture", "open": true,
                "lastOpened": 0, "createdAt": 0
            }))
            .unwrap();
            let element = if self.chips {
                crate::ui::tabs::chip::chip(
                    &session,
                    index == 0,
                    window,
                    false,
                    false,
                    false,
                    120.,
                    70.,
                    &self.tx,
                    p,
                )
            } else {
                crate::ui::sessions::row::session_row(
                    &session, 240., false, None, false, &self.tx, p,
                )
            };
            clip = clip.child(
                div()
                    .debug_selector(move || id.to_string())
                    .flex()
                    .flex_shrink_0()
                    .w(px(if self.chips { 127. } else { 240. }))
                    .h(px(if self.chips { 28. } else { 24. }))
                    .overflow_hidden()
                    .child(element),
            );
        }
        clip
    }
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn assert_rings_survive_clip(cx: &mut TestAppContext, chips: bool) {
    let _guard = keymap_tests::SERIAL
        .lock()
        .unwrap_or_else(|err| err.into_inner());
    HANDLES.lock().unwrap().clear();
    PENDING.lock().unwrap().clear();
    *FRAME.lock().unwrap() = (false, None);
    cx.update(crate::startup::configure);
    let (tx, _rx) = smol::channel::unbounded();
    let (_, cx) = cx.add_window_view(|_, _| ClippedSessions { chips, tx });
    for _ in 0..3 {
        draw(cx);
    }
    // Настоящая клавиатурная модальность; затем выбираем каждую позицию списка.
    cx.update(|window, cx| window.focus_next(cx));
    for id in ITEMS {
        let key = format!("{}:{id}", if chips { "chip" } else { "srow" });
        assert!(cx.update(|window, cx| focus_id(&key, window, cx)));
        draw(cx);
        let bounds = cx.debug_bounds(id).unwrap();
        cx.update(|window, _| {
            let clip = bounds.scale(window.scale_factor());
            let accent = crate::colors::rgba(kamin_theme::ThemeKind::Dark.palette().accent_primary);
            let quads: Vec<_> = window
                .painted_quads()
                .into_iter()
                .filter(|q| {
                    q.border_color == accent.into() && q.border_widths.top > px(0.).scale(1.)
                })
                .collect();
            assert!(!quads.is_empty(), "{key}: ring must be painted");
            let ring = quads[0].bounds;
            assert_eq!(ring.intersect(&clip), ring, "{key}: ring exceeds item clip");
            let half_border = px(RING / 2.).scale(window.scale_factor());
            let center = ring.center();
            for edge in [
                point(ring.left() + half_border, center.y),
                point(ring.right() - half_border, center.y),
                point(center.x, ring.top() + half_border),
                point(center.x, ring.bottom() - half_border),
            ] {
                assert!(
                    quads.iter().any(|q| q.content_mask.bounds.contains(&edge)),
                    "{key}: a ring side is clipped at {edge:?}"
                );
            }
        });
    }
}

#[gpui::test]
fn session_rows_paint_complete_rings_including_first_and_last(cx: &mut TestAppContext) {
    assert_rings_survive_clip(cx, false);
}

#[gpui::test]
fn tab_chips_paint_complete_rings_including_first_and_last(cx: &mut TestAppContext) {
    assert_rings_survive_clip(cx, true);
}

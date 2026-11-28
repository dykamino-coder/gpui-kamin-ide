//! Demand survives reentry and ends when high-rate input sustain expires.

use super::*;
use crate::{Empty, Render, RequestFrameOptions, TestAppContext};

struct EmptyView;

impl Render for EmptyView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

#[gpui::test]
fn reentrant_forced_presentation_is_retained_without_borrowing_app(cx: &mut TestAppContext) {
    cx.app.borrow_mut().mode = crate::app::GpuiMode::Production;
    let window = cx.add_window(|_, _| EmptyView);
    let platform = cx.test_window(window.into());
    platform.simulate_frame_request(RequestFrameOptions::default());
    let draws = window
        .update(cx, |_, window, _| {
            window
                .frame_duration_snapshot()
                .draw_duration_histogram
                .len()
        })
        .unwrap();
    let wakes = platform.frame_wake_count();
    let arena = RefCell::new(Arena::new(1024));
    {
        let _borrowed_app = cx.app.borrow_mut();
        let draw = ElementArenaScope::enter(&arena);
        platform.simulate_frame_request(RequestFrameOptions {
            force_render: true,
            require_presentation: true,
            ..Default::default()
        });
        drop(draw.exit(&arena));
    }
    assert!(platform.frame_wake_count() > wakes);
    window
        .update(cx, |_, window, _| assert!(window.needs_present.get()))
        .unwrap();
    platform.simulate_frame_request(RequestFrameOptions::default());
    window
        .update(cx, |_, window, _| {
            assert!(!window.needs_present.get());
            assert_eq!(
                window
                    .frame_duration_snapshot()
                    .draw_duration_histogram
                    .len(),
                draws + 1
            );
        })
        .unwrap();
    platform.simulate_frame_request(RequestFrameOptions::default());
    window
        .update(cx, |_, window, _| {
            assert_eq!(
                window
                    .frame_duration_snapshot()
                    .draw_duration_histogram
                    .len(),
                draws + 1
            )
        })
        .unwrap();
}

#[cfg(feature = "profiler")]
#[gpui::test]
fn high_rate_input_presents_and_rearms_until_sustain_ends(cx: &mut TestAppContext) {
    use crate::profiler::journal::{self, ForegroundEvent, ForegroundJournalEntry as Entry};
    cx.app.borrow_mut().mode = crate::app::GpuiMode::Production;
    let window = cx.add_window(|_, _| EmptyView);
    let platform = cx.test_window(window.into());
    platform.simulate_frame_request(RequestFrameOptions::default());
    let (journal, _guard) = journal::install_test_foreground_journal(256, 4);
    let mut collector = journal.collector();
    window
        .update(cx, |_, window, _| {
            window.input_rate_tracker.borrow_mut().sustain_until =
                Instant::now() + Duration::from_secs(3600)
        })
        .unwrap();
    let wakes = platform.frame_wake_count();
    platform.simulate_frame_request(RequestFrameOptions::default());
    assert!(platform.frame_wake_count() > wakes);
    let batch = collector.collect_unseen();
    assert_eq!(batch.lost, 0);
    assert!(
        batch
            .entries
            .iter()
            .any(|entry| matches!(entry, Entry::Event(ForegroundEvent::Present(_))))
    );
    assert!(
        !batch
            .entries
            .iter()
            .any(|entry| matches!(entry, Entry::Event(ForegroundEvent::Draw(_))))
    );
    window
        .update(cx, |_, window, _| {
            window.input_rate_tracker.borrow_mut().sustain_until = Instant::now()
        })
        .unwrap();
    let wakes = platform.frame_wake_count();
    platform.simulate_frame_request(RequestFrameOptions::default());
    assert_eq!(platform.frame_wake_count(), wakes);
    assert!(
        !collector
            .collect_unseen()
            .entries
            .iter()
            .any(|entry| matches!(entry, Entry::Event(ForegroundEvent::Present(_))))
    );
}

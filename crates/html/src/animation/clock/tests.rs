//! Shared frame time, source selection and retained-key regression tests.
use super::*;

fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}

#[test]
fn repeated_layout_reads_one_timestamp_and_keeps_diagnostics() {
    let clock = Clock::default();
    assert_eq!(clock.sample_with(1, 5, None, || ms(10)).elapsed, ms(10));
    clock.keyframe(true);
    let frame = clock.sample_with(1, 5, None, || panic!("second clock read"));
    assert_eq!(frame.elapsed, ms(10));
    assert_eq!(frame.keyframe_samples, 1);
    assert!(frame.keyframe_requested);
    assert_eq!(clock.frame(), Some(frame));
    let next = clock.sample_with(1, 6, None, || ms(30));
    assert_eq!(next.elapsed, ms(30));
    assert_eq!(next.keyframe_samples, 0);
    assert!(!next.keyframe_requested);
}

#[test]
fn window_identity_and_documents_do_not_share_cached_samples() {
    let clock = Clock::default();
    assert_eq!(clock.sample_with(1, 1, None, || ms(10)).elapsed, ms(10));
    assert_eq!(clock.sample_with(2, 1, None, || ms(15)).elapsed, ms(15));
    let other = Clock::default();
    other.set_elapsed(Some(ms(99)));
    assert_eq!(other.sample_with(1, 1, None, || panic!()).elapsed, ms(99));
    assert_eq!(clock.frame().unwrap().elapsed, ms(15));
}

#[test]
fn snapshot_precedence_seek_and_retained_invalidation_are_explicit() {
    let clock = Clock::default();
    let before = clock.retained_key();
    clock.set_elapsed(Some(ms(900)));
    assert_ne!(before, clock.retained_key());
    let same = clock.retained_key();
    clock.set_elapsed(Some(ms(900)));
    assert_eq!(same, clock.retained_key());
    let frame = clock.sample_with(1, 1, Some(ms(200)), || panic!());
    assert_eq!(frame.source, AnimationTimeSource::DocumentSnapshot);
    assert_eq!(frame.elapsed, ms(900));
    clock.set_elapsed(Some(ms(10)));
    assert_eq!(clock.sample_with(1, 1, None, || panic!()).elapsed, ms(10));
    clock.set_elapsed(None);
    let legacy = clock.sample_with(1, 1, Some(ms(200)), || panic!());
    assert_eq!(legacy.source, AnimationTimeSource::LegacySnapshot);
    assert_eq!(legacy.elapsed, ms(200));
    assert_eq!(
        clock.sample_with(1, 1, None, || ms(300)).source,
        AnimationTimeSource::Live
    );
}

#[test]
fn fixed_time_is_held_across_frames_and_reading_does_not_request_frames() {
    let clock = Clock::default();
    clock.set_elapsed(Some(Duration::ZERO));
    for serial in 0..3 {
        let frame = clock.sample_with(1, serial, None, || panic!());
        assert_eq!(frame.elapsed, Duration::ZERO);
        assert!(!frame.keyframe_requested && !frame.transition_requested);
        clock.transition(false, false);
        clock.hover_changed();
        assert!(clock.frame().unwrap().hover_target_changed);
    }
}

#[test]
fn static_render_key_ignores_steps_until_an_effect_is_observed() {
    let clock = Clock::default();
    let static_key = clock.render_key();
    clock.set_elapsed(Some(ms(17)));
    assert_eq!(clock.render_key(), static_key);
    clock.sample_with(1, 1, None, || panic!());
    clock.keyframe(false);
    let animated_key = clock.render_key();
    clock.set_elapsed(Some(ms(34)));
    assert_ne!(clock.render_key(), animated_key);
    let replay = clock.sample_with(1, 2, None, || panic!());
    assert!(replay.time_dependent);
    assert_eq!(replay.keyframe_samples, 0);
}

#[test]
fn idle_transition_does_not_consume_time_but_running_snapshot_does() {
    let clock = Clock::default();
    clock.set_elapsed(Some(ms(0)));
    clock.sample_with(1, 1, None, || panic!());
    let static_key = clock.render_key();
    clock.transition(false, false);
    let idle = clock.frame().unwrap();
    assert_eq!(idle.transition_samples, 1);
    assert!(!idle.time_dependent);
    clock.set_elapsed(Some(ms(17)));
    assert_eq!(clock.render_key(), static_key);
    clock.sample_with(1, 2, None, || panic!());
    clock.transition(true, false);
    let running = clock.frame().unwrap();
    assert!(running.time_dependent);
    assert!(
        !running.transition_requested,
        "stopped clocks do not self-tick"
    );
    let active_key = clock.render_key();
    clock.set_elapsed(Some(ms(34)));
    assert_ne!(clock.render_key(), active_key);
}

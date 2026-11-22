use super::*;

/// Only a whole step down on the root counts as a wheel sample.
#[test]
fn a_wheel_sample_requires_the_root_to_scroll_by_the_step() {
    let max = Some((0., 9000.));
    let ok = |from, to, step, clamp| scrolled(Some(from), Some(to), step, max, clamp).is_ok();
    assert!(ok((0., 0.), (0., WHEEL_STEP), WHEEL_STEP, false));
    assert!(ok((0., 240.), (0., 120.), -WHEEL_STEP, false));
    assert!(!ok((0., 240.), (0., 240.), WHEEL_STEP, false));
    assert!(!ok((0., 0.), (0., 40.), WHEEL_STEP, false));
    assert!(!ok((0., 0.), (5., WHEEL_STEP), WHEEL_STEP, false));
    assert!(scrolled(None, Some((0., WHEEL_STEP)), WHEEL_STEP, max, false).is_err());
    // `--scroll-back`: the document's ends clamp a step, nothing else does.
    assert!(ok((0., 8800.), (0., 9000.), 600., true));
    assert!(ok((0., 300.), (0., 0.), -600., true));
    assert!(!ok((0., 8800.), (0., 9000.), 600., false));
    assert!(!ok((0., 0.), (0., 0.), 600., true));
}

#[test]
fn relayout_input_alternates_without_leaving_the_supported_viewport_range() {
    assert_eq!(viewport((800, 600), true), (801, 600));
    assert_eq!(viewport((800, 600), false), (800, 600));
    assert_eq!(viewport((8192, 1), true), (8191, 1));
    assert_eq!(viewport((1, 8192), true), (2, 8192));
}

#[test]
fn counter_reset_cannot_masquerade_as_zero_work() {
    assert!(delta((5, 9), (1, 9)).is_err());
    assert!(delta((5, 9), (5, 0)).is_err());
    assert_eq!(delta((5, 9), (6, 9)).unwrap(), (1, 0));
    assert_eq!(delta((5, 9), (5, 10)).unwrap(), (0, 1));
}

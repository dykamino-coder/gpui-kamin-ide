use super::*;

/// Only a whole step down on the root counts as a wheel sample.
#[test]
fn a_wheel_sample_requires_the_root_to_scroll_by_the_step() {
    assert!(scrolled(Some((0., 0.)), Some((0., WHEEL_STEP)), Some((0., 9000.))).is_ok());
    assert!(scrolled(Some((0., 240.)), Some((0., 240.)), Some((0., 9000.))).is_err());
    assert!(scrolled(Some((0., 0.)), Some((0., 40.)), Some((0., 9000.))).is_err());
    assert!(scrolled(Some((0., 0.)), Some((5., WHEEL_STEP)), Some((0., 9000.))).is_err());
    assert!(scrolled(None, Some((0., WHEEL_STEP)), Some((0., 9000.))).is_err());
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

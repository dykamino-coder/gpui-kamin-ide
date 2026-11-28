//! Controlled-clock curves reach the same positions at every display refresh rate.

use super::*;

fn time(ms: u64) -> Duration {
    Duration::from_millis(ms)
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-4, "{a} != {b}");
}

#[test]
fn wheel_eases_and_finishes_exactly() {
    let motion = Motion::new([0.; 2], [0.; 2], [0., -120.], time(0), Source::Wheel);
    close(motion.sample(time(0)).position[1], 0.);
    close(motion.sample(time(100)).position[1], -60.);
    let early = motion.sample(time(25));
    assert!(early.position[1] > -15. && early.position[1] < 0.);
    assert!(!motion.sample(time(199)).done);
    let end = motion.sample(time(200));
    assert!(end.done);
    assert_eq!(end.position, [0., -120.]);
    assert_eq!(end.velocity, [0.; 2]);
}

#[test]
fn retargeting_preserves_position_and_velocity_including_reversal() {
    let old = Motion::new([0.; 2], [0.; 2], [-80., -120.], time(0), Source::Wheel);
    let at = old.sample(time(70));
    for target in [[-160., -240.], [0.; 2], at.position] {
        let next = Motion::new(at.position, at.velocity, target, time(70), Source::Wheel);
        let start = next.sample(time(70));
        for axis in 0..2 {
            close(start.position[axis], at.position[axis]);
            close(start.velocity[axis], at.velocity[axis]);
        }
        assert_eq!(next.sample(time(1000)).position, target);
    }
}

#[test]
fn display_frequency_and_idle_gap_do_not_change_samples() {
    let motion = Motion::new([0.; 2], [0.; 2], [480., 0.], time(1000), Source::Wheel);
    for hz in [60., 120., 144.] {
        for frame in 0..8 {
            let t = Duration::from_secs_f64(1. + frame as f64 / hz);
            let sample = motion.sample(t);
            assert!(sample.position[0] >= 0. && sample.position[0] <= 480.);
        }
    }
    close(motion.sample(time(1050)).position[0], 240.);
    assert_eq!(motion.sample(time(4000)).position, [480., 0.]);
}

#[test]
fn source_policy_and_large_coordinates() {
    let keyboard = Motion::new([0.; 2], [0.; 2], [0., -300.], time(0), Source::Keyboard);
    assert!(!keyboard.sample(time(149)).done);
    assert!(keyboard.sample(time(150)).done);
    let script = Motion::new(
        [0., -1e7],
        [0.; 2],
        [0., -2e7],
        time(0),
        Source::Programmatic,
    );
    assert!(!script.sample(time(499)).done);
    assert_eq!(script.sample(time(500)).position, [0., -2e7]);
}

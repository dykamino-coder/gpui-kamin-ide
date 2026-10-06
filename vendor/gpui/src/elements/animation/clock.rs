//! Optional elapsed-time control for reproducible animation snapshots.

use super::{Animation, AnimationState};
use std::time::{Duration, Instant};

/// Sample every animation at this elapsed time instead of its wall clock.
/// Removing this application global restores ordinary animation timing.
/// Intended for reproducible snapshots and timeline previews.
#[derive(Clone, Copy, Debug)]
pub struct AnimationElapsedTime(pub Duration);

impl crate::Global for AnimationElapsedTime {}

pub(super) fn advance(
    animations: &[Animation],
    state: &mut AnimationState,
    elapsed: Option<Duration>,
) -> (usize, f32, bool) {
    if let Some(elapsed) = elapsed {
        let (index, delta, done) = at_elapsed(animations, elapsed);
        return (index, (animations[index].easing)(delta), done);
    }
    // Keep the existing live scheduler, including its chain boundary behavior.
    let index = state.animation_ix;
    let mut delta = state.start.elapsed().as_secs_f32() / animations[index].duration.as_secs_f32();
    let mut done = false;
    if delta > 1.0 {
        if animations[index].oneshot {
            if index >= animations.len() - 1 {
                done = true;
            } else {
                state.start = Instant::now();
                state.animation_ix += 1;
            }
            delta = 1.0;
        } else {
            delta %= 1.0;
        }
    }
    (index, (animations[index].easing)(delta), done)
}

fn at_elapsed(animations: &[Animation], elapsed: Duration) -> (usize, f32, bool) {
    let mut seconds = elapsed.as_secs_f64();
    for (index, animation) in animations.iter().enumerate() {
        let duration = animation.duration.as_secs_f64();
        let last = index == animations.len() - 1;
        if animation.oneshot && !last && seconds > duration {
            seconds -= duration;
            continue;
        }
        if duration == 0.0 {
            return (index, 1.0, animation.oneshot);
        }
        let raw = seconds / duration;
        let delta = if raw > 1.0 {
            if animation.oneshot { 1.0 } else { raw % 1.0 }
        } else {
            raw
        };
        return (index, delta as f32, animation.oneshot && last && raw > 1.0);
    }
    unreachable!("AnimationElement requires a nonempty animation chain")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeating_snapshot_preserves_the_animation_duration_and_easing() {
        let animations = [Animation::new(Duration::from_millis(500))
            .repeat()
            .with_easing(|t| t * t)];
        let mut state = AnimationState {
            start: Instant::now(),
            animation_ix: 0,
        };
        for _ in 0..3 {
            assert_eq!(
                advance(&animations, &mut state, Some(Duration::from_millis(750))),
                (0, 0.25, false)
            );
            assert_eq!(state.animation_ix, 0);
        }
        assert_eq!(
            at_elapsed(&animations, Duration::from_millis(500)),
            (0, 1.0, false)
        );
    }

    #[test]
    fn chain_snapshots_select_the_same_phase_independently_of_previous_frames() {
        let animations = [
            Animation::new(Duration::from_millis(200)),
            Animation::new(Duration::from_millis(400)),
        ];
        assert_eq!(
            at_elapsed(&animations, Duration::from_millis(100)),
            (0, 0.5, false)
        );
        assert_eq!(
            at_elapsed(&animations, Duration::from_millis(200)),
            (0, 1.0, false)
        );
        assert_eq!(
            at_elapsed(&animations, Duration::from_millis(300)),
            (1, 0.25, false)
        );
        assert_eq!(
            at_elapsed(&animations, Duration::from_millis(700)),
            (1, 1.0, true)
        );
        let mut state = AnimationState {
            start: Instant::now(),
            animation_ix: 1,
        };
        assert_eq!(
            advance(&animations, &mut state, Some(Duration::from_millis(100))),
            (0, 0.5, false)
        );
        assert_eq!(state.animation_ix, 1);
    }
}

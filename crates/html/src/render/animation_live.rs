//! Live keyframes rebuild the actual formatting node, rather than sizing an extra box.
use super::{RenderOpts, animation_frame, bake_frozen, element, frame_at};
use crate::computed::{AnimSpec, Computed};
use crate::dom::Element;
use gpui::{AnyElement, IntoElement};

fn animation(spec: &AnimSpec) -> gpui::Animation {
    let alternating = spec.infinite && spec.alternate;
    let period = spec.seconds.max(0.05) * if alternating { 2.0 } else { 1.0 };
    let mut animation = gpui::Animation::new(std::time::Duration::from_secs_f32(period));
    if spec.infinite {
        animation = animation.repeat();
    }
    if alternating {
        // CSS Animations §3.6: each traversal lasts one iteration duration.
        // https://www.w3.org/TR/css-animations-1/#animation-direction
        animation = animation.with_easing(|delta| {
            if delta <= 0.5 {
                delta * 2.0
            } else {
                (1.0 - delta) * 2.0
            }
        });
    }
    animation
}

pub(super) fn animated(e: &Element, inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    let (Some(frames), Some(spec)) = (e.anim.clone(), e.style.animation.clone()) else {
        return element(e, inherited, opts);
    };
    if let Some(frozen) = bake_frozen(e, false) {
        return element(&frozen, inherited, opts);
    }
    let base = element(e, inherited, opts);
    let id = gpui::ElementId::Integer(e.node_id as u64);
    let source = e.clone();
    let inherited = inherited.clone();
    let opts = opts.clone();
    gpui::AnimationExt::with_animation(base, id, animation(&spec), move |_, delta| {
        let frame = frame_at(&frames, delta);
        let sampled = animation_frame::sample(&source, &frame, false);
        element(&sampled, &inherited, &opts)
    })
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(infinite: bool) -> AnimSpec {
        AnimSpec {
            name: "resize".into(),
            seconds: 0.5,
            infinite,
            alternate: true,
            delay: 0.0,
            paused: false,
            names: Vec::new(),
        }
    }

    #[test]
    fn alternate_repeat_starts_at_first_keyframe_and_gives_each_leg_its_duration() {
        let animation = animation(&spec(true));
        assert_eq!(animation.duration.as_secs_f32(), 1.0);
        assert!(!animation.oneshot);
        for (time, progress) in [(0.0, 0.0), (0.25, 0.5), (0.5, 1.0), (0.75, 0.5), (1.0, 0.0)] {
            assert_eq!((animation.easing)(time), progress);
        }
    }

    #[test]
    fn single_alternate_iteration_plays_forward_once() {
        let animation = animation(&spec(false));
        assert_eq!(animation.duration.as_secs_f32(), 0.5);
        assert!(animation.oneshot);
        assert_eq!((animation.easing)(0.0), 0.0);
        assert_eq!((animation.easing)(1.0), 1.0);
    }
}

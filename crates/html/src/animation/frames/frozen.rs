//! Запекание остановленного animation frame в дерево узлов.

use super::sample_at;
use crate::animation::animation_frame;
use crate::dom::Element;
use crate::values::ValueCx;

/// A frozen animation (`AnimSpec::frozen`) baked into a copy of the element:
/// the frame at `(-delay)/duration` is written straight into the style and the
/// normal pipeline runs on it. `transforms` says whether `rotate`/`scale`/
/// `transform` are carried too: `transformed()` builds their matrix OUTSIDE the
/// element, from the style it is given. Important declarations are already layered over
/// each frame when the frames are resolved (css-cascade-5 §cascade-origin).
pub(crate) fn bake_frozen(e: &Element, transforms: bool, vcx: ValueCx<'_>) -> Option<Element> {
    let own = e.style.used();
    let (Some(frames), Some(spec)) = (e.anim.as_ref(), own.reset_context.animation.as_ref()) else {
        return None;
    };
    if !spec.frozen() {
        return None;
    }
    let frame = sample_at(frames, spec, 0.0, vcx)?;
    Some(animation_frame::sample(e, &frame, transforms, vcx))
}

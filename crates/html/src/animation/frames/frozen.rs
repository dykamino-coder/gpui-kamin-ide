//! Запекание остановленного animation frame в дерево узлов.

use super::sample_at;
use crate::animation::animation_frame;
use crate::dom::Element;
use crate::values::ValueCx;

/// Остановленная анимация (`AnimSpec::frozen`), запечённая в копию элемента:
/// кадр `(-delay)/duration` подставляется прямо в стиль, и дальше работает
/// весь обычный конвейер. `transforms` — нести ли и `rotate`/`scale`/
/// `transform`: их матрицу строит `transformed()` СНАРУЖИ элемента, от стиля,
/// который ему передан. Important declarations are already layered over
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

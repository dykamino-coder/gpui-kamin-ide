//! Запекание остановленного animation frame в дерево узлов.

use super::frame_at;
use crate::animation::animation_frame;
use crate::dom::Element;

/// Остановленная анимация (`AnimSpec::frozen`), запечённая в копию элемента:
/// кадр `(-delay)/duration` подставляется прямо в стиль, и дальше работает
/// весь обычный конвейер. `transforms` — нести ли и `rotate`/`scale`/
/// `transform`: их матрицу строит `transformed()` СНАРУЖИ элемента, от стиля,
/// который ему передан. Блочный путь (`transformed(animated(e), &e.style)`)
/// их не берёт: `!important` у нас кадры не перекрывает, а обязан
/// (css-cascade-5 §cascade-origin) — `translation-animation-on-important-
/// property` с `transform: none !important` уехала бы на середину пути.
pub(crate) fn bake_frozen(e: &Element, transforms: bool) -> Option<Element> {
    let (Some(frames), Some(spec)) = (e.anim.as_ref(), e.style.animation.as_ref()) else {
        return None;
    };
    if !spec.frozen() {
        return None;
    }
    Some(animation_frame::sample(
        e,
        &frame_at(frames, spec.frozen_t()),
        transforms,
    ))
}

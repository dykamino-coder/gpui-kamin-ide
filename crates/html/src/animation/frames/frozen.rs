//! Запекание остановленного animation frame в дерево узлов.

use super::frame_at;
use crate::animation::animation_frame;
use crate::dom::Element;
use crate::style::computed::{AnimSpec, Computed};

/// Остановленная анимация (`AnimSpec::frozen`), запечённая в копию элемента:
/// кадр `(-delay)/duration` подставляется прямо в стиль, и дальше работает
/// весь обычный конвейер. `transforms` — нести ли и `rotate`/`scale`/
/// `transform`: их матрицу строит `transformed()` СНАРУЖИ элемента, от стиля,
/// который ему передан. Остановленный блочный путь пока
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
    let frame = sample_at(frames, spec, 0.0)?;
    Some(animation_frame::sample(e, &frame, transforms))
}

/// A backwards fill holds the endpoint without applying its easing.
pub(crate) fn sample_at(
    frames: &[(f32, Computed)],
    spec: &AnimSpec,
    elapsed: f64,
) -> Option<Computed> {
    let progress = spec.progress_at(elapsed)?;
    if elapsed < f64::from(spec.delay) {
        return Some(
            if progress == 0.0 {
                &frames.first()?.1
            } else {
                &frames.last()?.1
            }
            .clone(),
        );
    }
    Some(frame_at(frames, progress))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fill_phases_and_large_negative_delay() {
        let mut style = Computed::default();
        style.apply_one("animation", "a 1s -1e16s infinite paused");
        assert_eq!(style.animation.as_ref().unwrap().frozen_t(), 0.0);
        style.apply_one("animation", "a 1s 2s paused");
        let frames = vec![(0.0, style.clone()), (1.0, style.clone())];
        assert!(sample_at(&frames, style.animation.as_ref().unwrap(), 0.0).is_none());
        style.apply_one("animation-fill-mode", "both");
        let spec = style.animation.as_ref().unwrap();
        assert_eq!(spec.progress_at(0.0), Some(0.0));
        assert_eq!(spec.progress_at(3.0), Some(1.0));
    }
}

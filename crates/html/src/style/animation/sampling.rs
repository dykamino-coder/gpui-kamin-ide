//! Выборка CSS-кадра по времени и фазе заполнения анимации.

use super::interpolation::frame_at;
use crate::document::state::ValueCx;
use crate::style::computed::{AnimSpec, Computed};

/// A backwards fill holds the endpoint without applying its easing.
pub(crate) fn sample_at(
    frames: &[(f32, Computed)],
    spec: &AnimSpec,
    elapsed: f64,
    vcx: ValueCx<'_>,
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
    Some(frame_at(frames, progress, vcx))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fill_phases_and_large_negative_delay() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut style = Computed::default();
        style.apply_one("animation", "a 1s -1e16s infinite paused", vcx);
        assert_eq!(style.animation.as_ref().unwrap().frozen_t(), 0.0);
        style.apply_one("animation", "a 1s 2s paused", vcx);
        let frames = vec![(0.0, style.clone()), (1.0, style.clone())];
        assert!(sample_at(&frames, style.animation.as_ref().unwrap(), 0.0, vcx).is_none());
        style.apply_one("animation-fill-mode", "both", vcx);
        let spec = style.animation.as_ref().unwrap();
        assert_eq!(spec.progress_at(0.0), Some(0.0));
        assert_eq!(spec.progress_at(3.0), Some(1.0));
    }
}

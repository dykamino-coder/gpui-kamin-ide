//! Живая анимация по `@keyframes`: пересборка настоящего узла на каждом кадре.
//!
//! Вход — элемент с набором кадров (`anim`) и `AnimSpec`; выход — GPUI-элемент,
//! обёрнутый в `with_animation`. Остановленная (`frozen`) анимация запекается
//! в один стиль и собирается как обычный элемент. На живом кадре выборка
//! накладывается на исходный узел (`animation_frame::sample`), и собирается
//! сам форматирующий узел, а не отдельная коробка-обёртка: иначе анимируемые
//! размеры и цвета не доезжали бы до раскладки и детей.

use crate::animation::animation_frame;
use crate::animation::frames::{bake_frozen, sample_at};
use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::layout::options::RenderOpts;
use crate::paint::effects::transform::transformed;
use crate::style::computed::{AnimSpec, BoxStyle, Computed};
use gpui::{AnyElement, IntoElement};

fn animation(spec: &AnimSpec) -> gpui::Animation {
    let alternating = spec.infinite && spec.alternate;
    let period = if spec.infinite {
        spec.seconds.max(0.05) * if alternating { 2.0 } else { 1.0 }
    } else {
        (f64::from(spec.seconds) * spec.iterations + f64::from(spec.delay)).max(0.05) as f32
    };
    let mut animation = gpui::Animation::new(std::time::Duration::from_secs_f32(period));
    if spec.infinite {
        animation = animation.repeat();
    }
    animation
}

fn painted(e: &Element, inherited: &BoxStyle, opts: &RenderOpts) -> AnyElement {
    transformed(
        opts.cx.contracts.build.element(e, inherited, opts),
        &e.style.used(),
        inherited,
        opts.cx.vcx(),
    )
}

fn sample_live(
    source: &Element,
    frames: &[(f32, Computed)],
    spec: &AnimSpec,
    elapsed: f64,
    vcx: ValueCx<'_>,
) -> Element {
    sample_at(frames, spec, elapsed, vcx)
        .map(|frame| animation_frame::sample(source, &frame, true, vcx))
        .unwrap_or_else(|| source.clone())
}

pub(crate) fn animated(e: &Element, inherited: &BoxStyle, opts: &RenderOpts) -> AnyElement {
    let vcx = opts.cx.vcx();
    let (Some(frames), Some(spec)) = (e.anim.clone(), e.style.used().animation.clone()) else {
        return painted(e, inherited, opts);
    };
    if let Some(frozen) = bake_frozen(e, false, vcx) {
        return painted(&frozen, inherited, opts);
    }
    if spec.frozen() {
        return painted(e, inherited, opts);
    }
    let base = painted(e, inherited, opts);
    let id = e.state_id("html-animation", vcx);
    let source = e.clone();
    let inherited = inherited.clone();
    let opts = opts.clone();
    let timeline = animation(&spec);
    let period = timeline.duration.as_secs_f64();
    gpui::AnimationExt::with_animation(base, id, timeline, move |_, delta| {
        let vcx = opts.cx.vcx();
        let sampled = sample_live(&source, &frames, &spec, f64::from(delta) * period, vcx);
        painted(&sampled, &inherited, &opts)
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
            fill: 0,
            iterations: 1.0,
            reverse: false,
        }
    }

    #[test]
    fn alternate_repeat_starts_at_first_keyframe_and_gives_each_leg_its_duration() {
        let animation = animation(&spec(true));
        assert_eq!(animation.duration.as_secs_f32(), 1.0);
        assert!(!animation.oneshot);
        for (time, progress) in [(0.0, 0.0), (0.25, 0.5), (0.5, 1.0), (0.75, 0.5), (1.0, 0.0)] {
            assert_eq!(spec(true).progress_at(time), Some(progress));
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

    #[test]
    fn live_sample_reaches_layout_and_transform_and_expires_without_fill() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        use crate::style::values::value::Len;

        let mut source = crate::layout::table::anon::anon_element("div", vec![]);
        source
            .style
            .used_mut()
            .apply_one("animation", "move 1s linear", vcx);
        let own = source.style.used();
        let spec = own.animation.as_ref().unwrap();
        let mut start = source.style.cascaded().clone();
        start.apply_one("margin", "0px", vcx);
        start.apply_one("transform", "translateX(0px)", vcx);
        let mut end = source.style.cascaded().clone();
        end.apply_one("margin", "200px", vcx);
        end.apply_one("transform", "translateX(200px)", vcx);
        let frames = [(0.0, start), (1.0, end)];
        let period = animation(spec).duration.as_secs_f64();
        for (ms, x) in [(0, 0.0), (250, 50.0), (500, 100.0), (750, 150.0)] {
            let delta = (f64::from(ms) / 1000.0 / period) as f32;
            let sampled = sample_live(&source, &frames, spec, f64::from(delta) * period, vcx);
            assert_eq!(sampled.style.used().margin.left, Some(Len::Px(x)));
            assert_eq!(sampled.style.used().margin.top, Some(Len::Px(x)));
            assert_eq!(sampled.style.used().margin.right, Some(Len::Px(x)));
            assert_eq!(sampled.style.used().margin.bottom, Some(Len::Px(x)));
            assert_eq!(sampled.style.used().transform.unwrap().translate.0, x);
        }
        // GPUI clamps a completed one-shot to delta=1, including at 1500 ms.
        let expired = sample_live(&source, &frames, spec, period, vcx);
        assert_eq!(expired.style.used().margin, source.style.used().margin);
        assert_eq!(
            expired.style.used().transform,
            source.style.used().transform
        );
    }
}

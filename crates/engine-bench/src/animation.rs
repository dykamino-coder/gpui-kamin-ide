//! Explicit animation input and bounded per-document frame diagnostics.

use super::{args::Args, view::View};
use gpui::{AsyncApp, WindowHandle};
use kamin_html::AnimationFrame;
use serde_json::{Value, json};

pub(super) fn prepare(
    cx: &mut AsyncApp,
    window: WindowHandle<View>,
    args: &Args,
    index: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let elapsed = (args.mode != super::args::Mode::Smooth)
        .then_some(())
        .and_then(|()| args.animation_time(index));
    window.update(cx, |view, _, _| {
        view.animation_elapsed = elapsed;
        view.doc.set_animation_elapsed(elapsed);
    })?;
    Ok(())
}

pub(super) fn journal(frame: Option<AnimationFrame>) -> Value {
    let Some(frame) = frame else {
        return Value::Null;
    };
    json!({
        "window_id": frame.window_id,
        "frame_serial": frame.frame_serial,
        "source": frame.source.as_str(),
        "elapsed": {"seconds": frame.elapsed.as_secs(), "nanoseconds": frame.elapsed.subsec_nanos()},
        "elapsed_ms": frame.elapsed.as_secs_f64() * 1000.0,
        "keyframe_samples": frame.keyframe_samples,
        "transition_samples": frame.transition_samples,
        "time_dependent": frame.time_dependent,
        "next_frame_causes": {
            "keyframes": frame.keyframe_requested,
            "transition_progress": frame.transition_requested,
            "hover_target": frame.hover_target_changed,
        },
    })
}

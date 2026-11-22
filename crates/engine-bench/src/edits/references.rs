//! Fresh references: two for static variants, one per time step for effects.

use super::super::{
    animation_spool::Spool,
    args::Args,
    run::{Step, sample},
    view::View,
    work::Adapter,
};
use gpui::{AsyncApp, WindowHandle, frame_capture::CapturedFrame};
use kamin_html::{AnimationFrame, host::capture::CaptureSession, metrics::FontContext};
use std::{borrow::Cow, rc::Rc};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn samples_time(frame: Option<AnimationFrame>) -> bool {
    // Delayed or stopped effects still depend on time, even without a frame
    // request. A missing journal cannot prove that a document is static.
    frame.is_none_or(|frame| frame.keyframe_samples != 0 || frame.transition_samples != 0)
}

pub(super) enum References {
    Ordinary(Vec<Rc<CapturedFrame>>),
    Stepped(Spool),
}

impl References {
    pub(super) async fn prepare(
        cx: &mut AsyncApp,
        window: WindowHandle<View>,
        fonts: &FontContext,
        args: &Args,
        capture: &CaptureSession,
        luid: &Adapter,
        variants: [&str; 2],
    ) -> Result<Self> {
        let mut frames = Vec::with_capacity(2);
        let mut time_dependent = false;
        super::super::animation::prepare(cx, window, args, 0)?;
        for markup in variants {
            let (_, fresh, _) = sample(
                cx,
                window,
                fonts,
                args,
                Step::Cold(Some(markup)),
                capture,
                luid,
            )
            .await?;
            time_dependent |=
                samples_time(window.read_with(cx, |view, _| view.doc.animation_frame())?);
            frames.push(fresh.pixels);
        }
        let references = if args.animation_step.is_some() && time_dependent {
            let temporary = std::env::temp_dir();
            let directory = args.fresh_check_dir.as_deref().unwrap_or(&temporary);
            let mut spool = Spool::new(directory, args.runs)?;
            for index in 1..=args.runs {
                super::super::animation::prepare(cx, window, args, index)?;
                let (_, frame, _) = sample(
                    cx,
                    window,
                    fonts,
                    args,
                    Step::Cold(Some(variants[index % 2])),
                    capture,
                    luid,
                )
                .await?;
                spool.write(index, &frame.pixels)?;
            }
            Self::Stepped(spool)
        } else {
            // Every edit still advances time. Without consumers the reference
            // stays unchanged, so two frames suffice without disk storage.
            Self::Ordinary(frames)
        };
        super::super::animation::prepare(cx, window, args, 0)?;
        sample(
            cx,
            window,
            fonts,
            args,
            Step::Cold(Some(variants[0])),
            capture,
            luid,
        )
        .await?;
        Ok(references)
    }

    pub(super) fn get(&self, index: usize) -> Result<Cow<'_, CapturedFrame>> {
        match self {
            Self::Ordinary(frames) => Ok(Cow::Borrowed(frames[index % 2].as_ref())),
            Self::Stepped(spool) => Ok(Cow::Owned(spool.read(index)?)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kamin_html::AnimationTimeSource;

    #[test]
    fn only_proven_static_frames_can_share_references_across_clock_steps() {
        assert!(samples_time(None));
        let mut frame = AnimationFrame {
            window_id: 1,
            frame_serial: 1,
            source: AnimationTimeSource::DocumentSnapshot,
            elapsed: std::time::Duration::ZERO,
            keyframe_samples: 0,
            transition_samples: 0,
            keyframe_requested: false,
            transition_requested: false,
            hover_target_changed: false,
        };
        assert!(!samples_time(Some(frame)));
        frame.keyframe_samples = 1;
        assert!(
            samples_time(Some(frame)),
            "a delayed keyframe must not share a reference"
        );
        frame.keyframe_samples = 0;
        frame.transition_samples = 1;
        assert!(
            samples_time(Some(frame)),
            "a stopped transition must not share a reference"
        );
    }
}

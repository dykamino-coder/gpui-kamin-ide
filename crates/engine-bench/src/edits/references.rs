//! Independent immutable edit variants, with stepped clocks for active effects.

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
    // A missing journal cannot prove a static document. Delayed keyframes and
    // explicitly stopped clocks still consume time; idle transition rules do not.
    frame.is_none_or(|frame| frame.time_dependent)
}

pub(super) struct References {
    frames: Vec<Rc<CapturedFrame>>,
    timed: [bool; 2],
    spool: Option<Spool>,
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
        let mut references = Self {
            frames: Vec::with_capacity(2),
            timed: [false; 2],
            spool: None,
        };
        for (variant, markup) in variants.into_iter().enumerate() {
            super::super::animation::prepare(cx, window, args, 0)?;
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
            references.timed[variant] = args.animation_step.is_some()
                && samples_time(window.read_with(cx, |view, _| view.doc.animation_frame())?);
            references.frames.push(fresh.pixels);
            if !references.timed[variant] {
                continue;
            }
            if references.spool.is_none() {
                let temporary = std::env::temp_dir();
                let directory = args.fresh_check_dir.as_deref().unwrap_or(&temporary);
                references.spool = Some(Spool::new(directory, args.runs)?);
            }
            // Each variant is a fresh, immutable document, independent of the
            // measured edit path. Only its clock advances: never apply edits to
            // the reference, and never repeat parsing/style/layout construction.
            for index in (1..=args.runs).filter(|index| index % 2 == variant) {
                super::super::animation::prepare(cx, window, args, index)?;
                let (_, frame, _) =
                    sample(cx, window, fonts, args, Step::Warm, capture, luid).await?;
                references
                    .spool
                    .as_mut()
                    .unwrap()
                    .write(index, &frame.pixels)?;
            }
        }
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
        if self.timed[index % 2] {
            Ok(Cow::Owned(self.spool.as_ref().unwrap().read(index)?))
        } else {
            Ok(Cow::Borrowed(self.frames[index % 2].as_ref()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kamin_html::AnimationTimeSource;

    #[test]
    fn idle_transition_rules_share_references_but_matched_effects_do_not() {
        assert!(samples_time(None));
        let mut frame = AnimationFrame {
            window_id: 1,
            frame_serial: 1,
            source: AnimationTimeSource::DocumentSnapshot,
            elapsed: std::time::Duration::ZERO,
            keyframe_samples: 0,
            transition_samples: 10,
            time_dependent: false,
            keyframe_requested: false,
            transition_requested: false,
            hover_target_changed: false,
        };
        assert!(
            !samples_time(Some(frame)),
            "idle transition wrappers are static"
        );
        frame.time_dependent = true;
        assert!(
            samples_time(Some(frame)),
            "explicit time can advance an effect without a live frame request"
        );
    }
}

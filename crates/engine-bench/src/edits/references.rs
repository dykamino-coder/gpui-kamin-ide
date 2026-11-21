//! Prepare every fresh reference before measured edits can acquire keyed state.

use super::super::{
    animation_spool::Spool,
    args::Args,
    run::{Step, sample},
    view::View,
    work::Adapter,
};
use gpui::{AsyncApp, WindowHandle, frame_capture::CapturedFrame};
use kamin_html::{host::capture::CaptureSession, metrics::FontContext};
use std::{borrow::Cow, rc::Rc};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

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
        let references = if args.animation_step.is_some() {
            let directory = args
                .fresh_check_dir
                .as_deref()
                .ok_or("fixed-step edit requires a reference directory")?;
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
            let mut frames = Vec::with_capacity(2);
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
                frames.push(fresh.pixels);
            }
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

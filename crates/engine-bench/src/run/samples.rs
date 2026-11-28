//! CPU/GPU samples keep wall-clock timing independent of animation input.
use super::{Result, Step};
use crate::{accounting::Measured, args::Args, view::View, work};
use gpui::{AsyncApp, WindowHandle};
use kamin_html::{host::capture::CaptureSession, metrics::FontContext};

mod once;
use once::sample_once;

pub(crate) async fn sample(
    cx: &mut AsyncApp,
    window: WindowHandle<View>,
    fonts: &FontContext,
    args: &Args,
    step: Step<'_>,
    capture: &CaptureSession,
    luid: &super::work::Adapter,
) -> Result<(Measured, kamin_html::host::capture::Frame, work::Sample)> {
    let (measured, frame, work) =
        sample_with(cx, window, fonts, args, step, capture, luid, true).await?;
    Ok((
        measured,
        frame.ok_or("armed first frame was not captured")?,
        work,
    ))
}

/// `sample`; without `present` the frame may skip its presentation (an
/// edit that changed nothing on screen), and then no frame is captured.
/// With `--why` the frame's journal entry follows the sample (`why`).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn sample_with(
    cx: &mut AsyncApp,
    window: WindowHandle<View>,
    fonts: &FontContext,
    args: &Args,
    step: Step<'_>,
    capture: &CaptureSession,
    luid: &super::work::Adapter,
    present: bool,
) -> Result<(
    Measured,
    Option<kamin_html::host::capture::Frame>,
    work::Sample,
)> {
    let watch = crate::why::Watch::begin(args, cx, window, luid.journal_luid)?;
    let seq = capture.sequence();
    let out = sample_once(cx, window, fonts, args, step, capture, luid, present).await;
    if let Some(watch) = watch {
        let measurements = out.as_ref().ok().map(|out| (&out.0, &out.2));
        let error = out.as_ref().err().map(|e| e.to_string());
        // The raster of a frame this sample captured, not of an earlier one.
        let raster = (capture.sequence() > seq)
            .then(|| capture.raster_report())
            .flatten();
        watch.end(args, cx, window, step, measurements, raster, error)?;
    }
    out
}

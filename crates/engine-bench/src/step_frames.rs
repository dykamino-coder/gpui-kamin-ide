//! The step frames scroll mode keeps for its fresh-document check
//! (`scroll_fresh`), copied into buffers touched before the steps.
//!
//! A kept capture buffer is never freed back to the allocator, so the
//! readback of every next step would write a new frame-sized block whose
//! pages fault in first: the check would be timed in the steps it checks
//! (a wheel step on a software rasterizer measured about 3 ms slower).
//! A step instead copies its frame into a spare buffer after its sample, and
//! the capture buffer goes back to the allocator for the next readback, as
//! in the other modes.

use gpui::frame_capture::CapturedFrame;

/// One wheel step: its measured frame, the last settle frame (when the step
/// drew more frames before it settled) and the root offset it was drawn at.
pub(super) struct StepFrame {
    pub(super) measured: CapturedFrame,
    pub(super) settled: Option<CapturedFrame>,
    pub(super) offset: (f32, f32),
}

/// The kept steps and the touched buffers the next ones are copied into.
#[derive(Default)]
pub(super) struct StepFrames {
    pub(super) steps: Vec<StepFrame>,
    spare: Vec<Vec<u8>>,
    /// Steps the run measures: a measured and a settle frame each.
    expected: usize,
}

impl StepFrames {
    pub(super) fn new(expected: usize) -> Self {
        Self {
            expected,
            ..Self::default()
        }
    }

    /// Keep a copy of the measured frame of a step drawn at `offset`.
    pub(super) fn push(&mut self, frame: &CapturedFrame, offset: (f32, f32)) {
        let measured = self.copy(frame);
        self.steps.push(StepFrame {
            measured,
            settled: None,
            offset,
        });
    }

    /// Keep a copy of a settle frame of the last step (the last one wins).
    pub(super) fn settled(&mut self, frame: &CapturedFrame) {
        let copy = self.copy(frame);
        if let Some(last) = self.steps.last_mut()
            && let Some(old) = last.settled.replace(copy)
        {
            self.spare.push(old.bgra);
        }
    }

    fn copy(&mut self, frame: &CapturedFrame) -> CapturedFrame {
        let size = frame.bgra.len();
        if self.spare.is_empty() && self.steps.is_empty() {
            // Before the second step: every step keeps two frames at most.
            let count = 2 * self.expected.max(1) + 1;
            self.spare = (0..count).map(|_| touched(size)).collect();
        }
        let mut bgra = self.spare.pop().unwrap_or_else(|| touched(size));
        bgra.clear();
        bgra.extend_from_slice(&frame.bgra);
        CapturedFrame {
            width: frame.width,
            height: frame.height,
            bgra,
            context: frame.context.clone(),
        }
    }
}

/// A buffer whose pages are written: a zeroed allocation (`vec![0; n]`)
/// maps them only on the first write, in the next step's readback.
fn touched(size: usize) -> Vec<u8> {
    vec![0xff; size]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(value: u8) -> CapturedFrame {
        CapturedFrame {
            width: 2,
            height: 1,
            bgra: vec![value; 8],
            context: None,
        }
    }

    #[test]
    fn steps_keep_copies_and_the_last_settle_frame() {
        let mut frames = StepFrames::new(2);
        frames.push(&frame(1), (0., 40.));
        frames.settled(&frame(2));
        frames.settled(&frame(3));
        frames.push(&frame(4), (0., 80.));
        let kept: Vec<_> = frames
            .steps
            .iter()
            .map(|s| {
                (
                    s.measured.bgra[0],
                    s.settled.as_ref().map(|f| f.bgra[0]),
                    s.offset,
                )
            })
            .collect();
        assert_eq!(kept, vec![(1, Some(3), (0., 40.)), (4, None, (0., 80.))]);
    }
}

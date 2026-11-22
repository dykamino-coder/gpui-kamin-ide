//! CSS-вход и RGB-последовательности измерений; replay отличим от реальной раскладки.

use super::{
    args::{Args, Mode},
    pixels::Recorder,
    run::{Step, sample},
    view::View,
};
use gpui::{AsyncApp, WindowHandle};
use kamin_html::host::capture::{CaptureSession, Frame};
use kamin_html::metrics::FontContext;
use serde_json::{Value, json};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Scroll: one wheel step down, in CSS px, before each frame (a classic wheel
/// notch of three 40 px lines, as Chromium's kScrollbarPixelsPerLineStep).
pub(super) const WHEEL_STEP: f32 = 120.0;

/// Окно и входы прогона для проверок после замеров.
pub(super) type Run<'a, 'b> = (
    &'a mut AsyncApp,
    WindowHandle<View>,
    &'b FontContext,
    &'b Args,
    &'b CaptureSession,
    &'b super::work::Adapter,
);

pub(super) use input::keeps_step;
use input::{delta, memory_only, scrolled, viewport};
mod fresh;
mod input;
mod observe;

pub(super) struct Frames {
    pixels: Vec<Recorder>,
    base: (u32, u32),
    relayout: bool,
    scroll: bool,
    /// Scroll (`--scroll-back`): whether the next step goes down; it turns at
    /// either end of the root scroller, so long runs cross the document again.
    back_down: Option<bool>,
    /// The root scroll the step of the sample asks for, down positive.
    step: f32,
    /// The step of `--scroll-back` (`Args::scroll_step`).
    jump: f32,
    /// When the step's wheel event was dispatched (prepare).
    dispatched: Option<std::time::Instant>,
    before: Option<kamin_html::FrameCounters>,
    /// Contents replayed in the document before the frame (`Document::replayed_contents`).
    before_contents: u64,
    /// Root scroll offset before the wheel step of the sample (`scrolled`).
    wheel_from: Option<(f32, f32)>,
    /// Root-flow blocks replayed in the document before the frame (`Document::replayed_blocks`).
    before_blocks: u64,
    samples: Vec<Value>,
    /// Scroll: a copy of every step frame and its root offset (`step_frames`).
    steps: super::step_frames::StepFrames,
}

impl Frames {
    pub(super) fn new(args: &Args) -> Result<Self> {
        let relayout = args.mode == Mode::Relayout;
        let mut pixels = vec![Recorder::new(args.frame_diff_dir.as_deref())?];
        if relayout {
            let directory = args
                .frame_diff_dir
                .as_ref()
                .map(|p| p.join("alternate-viewport"));
            pixels.push(Recorder::new(directory.as_deref())?);
        }
        Ok(Self {
            pixels,
            base: args.viewport,
            relayout,
            scroll: args.mode == Mode::Scroll,
            back_down: args.scroll_back.then_some(true),
            step: WHEEL_STEP,
            jump: args.scroll_step,
            dispatched: None,
            before: None,
            before_contents: 0,
            wheel_from: None,
            before_blocks: 0,
            samples: Vec::new(),
            steps: super::step_frames::StepFrames::new(args.runs),
        })
    }

    pub(super) fn prepare(
        &mut self,
        cx: &mut AsyncApp,
        window: WindowHandle<View>,
        index: usize,
    ) -> Result<()> {
        window.update(cx, |view, window, cx| {
            if self.scroll {
                // Dispatched as a platform wheel event: the root scroller's handler moves it.
                let size = window.viewport_size();
                if let Some(down) = self.back_down {
                    // `--scroll-back`: down and up between the ends.
                    self.step = if down { self.jump } else { -self.jump };
                }
                let wheel = gpui::ScrollWheelEvent {
                    position: gpui::point(size.width / 2.0, size.height / 2.0),
                    delta: gpui::ScrollDelta::Pixels(gpui::point(
                        gpui::px(0.0),
                        gpui::px(-self.step),
                    )),
                    modifiers: gpui::Modifiers::default(),
                    touch_phase: gpui::TouchPhase::Moved,
                };
                self.wheel_from = view.doc.root_scroll_offset();
                self.dispatched = Some(std::time::Instant::now());
                window.dispatch_event(gpui::PlatformInput::ScrollWheel(wheel), cx);
            }
            if self.relayout {
                // Меняется только CSS-вход: HWND остаётся прежним и не получает WM_SIZE.
                view.viewport = Some(viewport(self.base, !index.is_multiple_of(2)));
            }
            self.before = Some(view.doc.frame_counters());
            self.before_contents = view.doc.replayed_contents();
            self.before_blocks = view.doc.replayed_blocks();
            view.doc.take_frame_misses();
        })?;
        Ok(())
    }

    /// Scroll: frames after the measured one until the document replays its frame (settled), so a
    /// deferred second build of the step counts with the step. At most `SETTLE_FRAMES` frames.
    pub(super) async fn settle(&mut self, run: Run<'_, '_>, step: Step<'_>) -> Result<()> {
        const SETTLE_FRAMES: usize = 8;
        let (cx, window, fonts, args, capture, luid) = run;
        if !self.scroll {
            return Ok(());
        }
        let (mut frames, mut builds, mut replays, mut settled) = (0, 0, 0, false);
        while frames < SETTLE_FRAMES && !settled {
            let before = window.read_with(cx, |view, _| view.doc.retained_frames())?;
            let (_, frame, _) = sample(cx, window, fonts, args, step, capture, luid).await?;
            if !memory_only() {
                self.steps.settled(&frame.pixels);
            }
            let after = window.read_with(cx, |view, _| view.doc.retained_frames())?;
            let (b, r) = delta(before, after)?;
            frames += 1;
            builds += b;
            replays += r;
            settled = b == 0 && r > 0;
        }
        if let Some(last) = self.samples.last_mut() {
            last["settle"] = json!({"frames": frames, "builds": builds, "replays": replays,
                "settled": settled, "present": "unavailable: the capture surface reports no present time"});
        }
        Ok(())
    }

    pub(super) fn report(&self) -> Value {
        let mut out = if self.relayout {
            let sequences: Vec<_> = self.pixels.iter().enumerate().map(|(group, pixels)| {
                json!({"css_viewport": viewport(self.base, group == 1), "rgb": pixels.report()})
            }).collect();
            json!({"frame_pixels_changed": sequences.iter().any(|s| s["rgb"]["frame_pixels_changed"] == true),
                "frame_pixel_sequences": sequences,
                "relayout_trigger": "alternating RenderOpts CSS viewport on the same Document; fixed capture surface",
                "relayout_viewport_sequence": [viewport(self.base, true), self.base]})
        } else {
            self.pixels[0].report()
        };
        out["document_frame_samples"] = json!(self.samples);
        out["warm_scope"] = json!("unchanged document: may replay without element build/layout");
        out
    }
}

#[cfg(test)]
mod tests;

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

use input::{delta, scrolled, viewport};
mod fresh;
mod input;

pub(super) struct Frames {
    pixels: Vec<Recorder>,
    base: (u32, u32),
    relayout: bool,
    scroll: bool,
    /// When the step's wheel event was dispatched (prepare).
    dispatched: Option<std::time::Instant>,
    before: (u64, u64),
    /// Сборки внутри `Window::draw` до кадра (`Document::builds_inside_draw`).
    before_inside: u64,
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
            dispatched: None,
            before: (0, 0),
            before_inside: 0,
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
                let wheel = gpui::ScrollWheelEvent {
                    position: gpui::point(size.width / 2.0, size.height / 2.0),
                    delta: gpui::ScrollDelta::Pixels(gpui::point(
                        gpui::px(0.0),
                        gpui::px(-WHEEL_STEP),
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
            self.before = view.doc.retained_frames();
            self.before_inside = view.doc.builds_inside_draw();
            self.before_contents = view.doc.replayed_contents();
            self.before_blocks = view.doc.replayed_blocks();
            view.doc.take_frame_misses();
        })?;
        Ok(())
    }

    pub(super) fn observe(
        &mut self,
        cx: &AsyncApp,
        window: WindowHandle<View>,
        frame: Frame,
        mode: &str,
        index: usize,
    ) -> Result<()> {
        let (after, inside, contents, blocks, offset) = window.read_with(cx, |view, _| {
            let doc = &view.doc;
            (
                doc.retained_frames(),
                doc.builds_inside_draw(),
                doc.replayed_contents(),
                doc.replayed_blocks(),
                (doc.root_scroll_offset(), doc.root_scroll_max()),
            )
        })?;
        let misses = window.read_with(cx, |view, _| view.doc.take_frame_misses())?;
        let cold = mode == "cold";
        let counts = delta(if cold { (0, 0) } else { self.before }, after)?;
        let inside = inside
            .checked_sub(if cold { 0 } else { self.before_inside })
            .ok_or("document inside-draw build counter regressed")?;
        // A frame built again within the same frame counts both builds.
        if self.relayout && (counts.0 == 0 || counts.1 != 0) {
            return Err(format!(
                "relayout requires a document build and no replay; received {counts:?}"
            )
            .into());
        }
        if self.scroll {
            scrolled(self.wheel_from.take(), offset.0, offset.1)?;
        }
        let group = usize::from(self.relayout && !index.is_multiple_of(2));
        self.samples.push(json!({"mode": mode, "sample_index": index,
            "css_viewport": viewport(self.base, group == 1), "builds": counts.0, "replays": counts.1,
            "wheel_step_px": if self.scroll { json!(WHEEL_STEP) } else { Value::Null },
            "root_scroll_offset": offset.0.map(|(x, y)| json!([x, y])),
            "dispatch_to_frame_ms": self.dispatched.take().map(|t| t.elapsed().as_secs_f64() * 1000.0),
            "builds_inside_draw": inside,
            "content_replays": contents.saturating_sub(if cold { 0 } else { self.before_contents }),
            "block_replays": blocks.saturating_sub(if cold { 0 } else { self.before_blocks }),
            "animation": super::animation::journal(window.read_with(cx, |view, _| view.doc.animation_frame())?),
            "frame_misses": misses.into_iter().map(|(k, n)| (k, json!(n))).collect::<serde_json::Map<_, _>>()}));
        if let (true, Some(offset)) = (self.scroll, offset.0) {
            self.steps.push(&frame.pixels, offset);
        }
        self.pixels[group].observe(frame.pixels, frame.seq, mode, index)
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
            self.steps.settled(&frame.pixels);
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

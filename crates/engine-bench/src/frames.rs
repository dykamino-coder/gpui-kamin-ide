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

fn viewport(base: (u32, u32), alternate: bool) -> (u32, u32) {
    if alternate {
        (if base.0 == 8192 { 8191 } else { base.0 + 1 }, base.1)
    } else {
        base
    }
}

/// A wheel sample counts only when the step moved the root scroller by the whole step:
/// an event a nested scroller took, or one at the document's end, scrolls no root.
type Offset = Option<(f32, f32)>;
fn scrolled(from: Offset, to: Offset, step: f32, max: Offset) -> Result<()> {
    let (Some(from), Some(to)) = (from, to) else {
        return Err("scroll mode needs the root scroller offset before and after the step".into());
    };
    if to.0 != from.0 || (to.1 - from.1 - step).abs() > 0.5 {
        return Err(format!(
            "the wheel step did not scroll the root by {step} px: {from:?} -> {to:?} \
             (root max offset {max:?}: a nested scroller took it, or the document ended)"
        )
        .into());
    }
    Ok(())
}

fn delta(before: (u64, u64), after: (u64, u64)) -> Result<(u64, u64)> {
    Ok((
        after
            .0
            .checked_sub(before.0)
            .ok_or("document build counter regressed")?,
        after
            .1
            .checked_sub(before.1)
            .ok_or("document replay counter regressed")?,
    ))
}

pub(super) struct Frames {
    pixels: Vec<Recorder>,
    base: (u32, u32),
    relayout: bool,
    scroll: bool,
    /// Scroll: the sample from which the steps go back up (`--scroll-back`).
    back_from: Option<usize>,
    /// The root scroll the step of the sample asks for, down positive.
    step: f32,
    /// The step of `--scroll-back` (`Args::scroll_step`).
    jump: f32,
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
    /// Scroll: every step frame and its root offset (`scroll_fresh`).
    steps: super::scroll_fresh::StepFrames,
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
            back_from: args.scroll_back.then_some(args.runs / 2),
            step: WHEEL_STEP,
            jump: args.scroll_step,
            dispatched: None,
            before: (0, 0),
            before_inside: 0,
            before_contents: 0,
            wheel_from: None,
            before_blocks: 0,
            samples: Vec::new(),
            steps: Vec::new(),
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
                let mut at = gpui::point(size.width / 2.0, size.height / 2.0);
                if let Some(from) = self.back_from {
                    // `--scroll-back`: down and then up, by the root's own
                    // scrollbar edge, which no nested scroller covers.
                    self.step = if index >= from { -self.jump } else { self.jump };
                    at.x = size.width - gpui::px(2.0);
                }
                let wheel = gpui::ScrollWheelEvent {
                    position: at,
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
        let (after, inside, contents, blocks, offset, (leaves, measures)) =
            window.read_with(cx, |view, _| {
                let doc = &view.doc;
                (
                    doc.retained_frames(),
                    doc.builds_inside_draw(),
                    doc.replayed_contents(),
                    doc.replayed_blocks(),
                    (doc.root_scroll_offset(), doc.root_scroll_max()),
                    (doc.offscreen_leaves(), doc.offscreen_measures()),
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
            match self.back_from {
                // The document's end clamps a direct step: only a root
                // scroller is required.
                Some(_) if offset.0.is_some() => {}
                _ => scrolled(self.wheel_from.take(), offset.0, self.step, offset.1)?,
            }
        }
        let group = usize::from(self.relayout && !index.is_multiple_of(2));
        let memory = self.scroll.then(super::host::memory_now).flatten();
        self.samples.push(json!({"mode": mode, "sample_index": index,
            "working_set_bytes": memory.map(|m| m.0), "private_bytes": memory.map(|m| m.1),
            "css_viewport": viewport(self.base, group == 1), "builds": counts.0, "replays": counts.1,
            "wheel_step_px": if self.scroll { json!(self.step) } else { Value::Null },
            "root_scroll_offset": offset.0.map(|(x, y)| json!([x, y])),
            "dispatch_to_frame_ms": self.dispatched.take().map(|t| t.elapsed().as_secs_f64() * 1000.0),
            "builds_inside_draw": inside, "offscreen_leaves_total": leaves, "offscreen_measures_total": measures,
            "content_replays": contents.saturating_sub(if cold { 0 } else { self.before_contents }),
            "block_replays": blocks.saturating_sub(if cold { 0 } else { self.before_blocks }),
            "frame_misses": misses.into_iter().map(|(k, n)| (k, json!(n))).collect::<serde_json::Map<_, _>>()}));
        if let (true, Some(offset)) = (self.scroll, offset.0) {
            let (measured, settled) = (frame.pixels.clone(), None);
            (self.steps).push(super::scroll_fresh::StepFrame {
                measured,
                settled,
                offset,
            })
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
            if let Some(last) = self.steps.last_mut() {
                last.settled = Some(frame.pixels);
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

    /// После чтения пика памяти и вне замеров: проверка relayout по свежему
    /// документу (`fresh`) и аудит устойчивой памяти (`memory::finish`).
    pub(super) async fn finish(self, report: &mut Value, run: Run<'_, '_>) -> Result<()> {
        let (cx, window, fonts, args, capture, luid) = run;
        self.fresh(report, (&mut *cx, window, fonts, args, capture, luid))
            .await?;
        if args.memory_settle_ms > 0 {
            if report["frame_pixels_changed"] != false {
                return Err("memory audit requires an unchanged RGB frame sequence".into());
            }
            report["memory_audit"] =
                super::memory::finish(cx, window, fonts, args, capture, luid).await?;
        }
        Ok(())
    }

    /// Relayout: кадр каждой ширины (все кадры группы равны первому, иначе
    /// `frame_pixels_changed`) совпадает по пикселям со свежим документом той
    /// же ширины.
    async fn fresh(self, report: &mut Value, run: Run<'_, '_>) -> Result<()> {
        if self.scroll {
            return super::scroll_fresh::check(self.steps, report, run).await;
        }
        let (cx, window, fonts, args, capture, luid) = run;
        if !self.relayout {
            return Ok(());
        }
        let mut checks = Vec::new();
        for (group, pixels) in self.pixels.iter().enumerate() {
            let Some(incremental) = pixels.first() else {
                continue;
            };
            let css = viewport(self.base, group == 1);
            window.update(cx, |view, _, _| view.viewport = Some(css))?;
            let (_, fresh, _) =
                sample(cx, window, fonts, args, Step::Cold(None), capture, luid).await?;
            let mut check = super::fresh_check::compare(
                incremental,
                &fresh.pixels,
                args.fresh_check_dir.as_deref(),
                &format!("relayout-{group}"),
            )?;
            check["css_viewport"] = json!(css);
            checks.push(check);
        }
        let mode = &mut report["modes"]["relayout"];
        mode["matches_fresh"] = json!(checks.iter().all(|c| c["different_rgb_pixels"] == 0));
        mode["fresh_checks"] = json!(checks);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only a whole step down on the root counts as a wheel sample.
    #[test]
    fn a_wheel_sample_requires_the_root_to_scroll_by_the_step() {
        let max = Some((0., 9000.));
        assert!(scrolled(Some((0., 0.)), Some((0., WHEEL_STEP)), WHEEL_STEP, max).is_ok());
        assert!(scrolled(Some((0., 240.)), Some((0., 120.)), -WHEEL_STEP, max).is_ok());
        assert!(scrolled(Some((0., 240.)), Some((0., 240.)), WHEEL_STEP, max).is_err());
        assert!(scrolled(Some((0., 0.)), Some((0., 40.)), WHEEL_STEP, max).is_err());
        assert!(scrolled(Some((0., 0.)), Some((5., WHEEL_STEP)), WHEEL_STEP, max).is_err());
        assert!(scrolled(None, Some((0., WHEEL_STEP)), WHEEL_STEP, max).is_err());
    }

    #[test]
    fn relayout_input_alternates_without_leaving_the_supported_viewport_range() {
        assert_eq!(viewport((800, 600), true), (801, 600));
        assert_eq!(viewport((800, 600), false), (800, 600));
        assert_eq!(viewport((8192, 1), true), (8191, 1));
        assert_eq!(viewport((1, 8192), true), (2, 8192));
    }

    #[test]
    fn counter_reset_cannot_masquerade_as_zero_work() {
        assert!(delta((5, 9), (1, 9)).is_err());
        assert!(delta((5, 9), (5, 0)).is_err());
        assert_eq!(delta((5, 9), (6, 9)).unwrap(), (1, 0));
        assert_eq!(delta((5, 9), (5, 10)).unwrap(), (0, 1));
    }
}

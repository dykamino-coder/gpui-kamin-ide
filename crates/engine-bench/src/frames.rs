//! CSS-вход и RGB-последовательности измерений; replay отличим от реальной раскладки.

use super::{
    args::{Args, Mode},
    pixels::{Recorder, detail, differing},
    run::{Step, sample},
    view::View,
};
use gpui::{AsyncApp, WindowHandle};
use kamin_html::host::capture::{CaptureSession, Frame};
use kamin_html::metrics::FontContext;
use serde_json::{Value, json};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Окно и входы прогона для проверок после замеров.
pub(super) type Run<'a, 'b> = (
    &'a mut AsyncApp,
    WindowHandle<View>,
    &'b FontContext,
    &'b Args,
    &'b CaptureSession,
    (i32, u32),
);

fn viewport(base: (u32, u32), alternate: bool) -> (u32, u32) {
    if alternate {
        (if base.0 == 8192 { 8191 } else { base.0 + 1 }, base.1)
    } else {
        base
    }
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
    before: (u64, u64),
    samples: Vec<Value>,
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
            before: (0, 0),
            samples: Vec::new(),
        })
    }

    pub(super) fn prepare(
        &mut self,
        cx: &mut AsyncApp,
        window: WindowHandle<View>,
        index: usize,
    ) -> Result<()> {
        window.update(cx, |view, _, _| {
            if self.relayout {
                // Меняется только CSS-вход: HWND остаётся прежним и не получает WM_SIZE.
                view.viewport = Some(viewport(self.base, !index.is_multiple_of(2)));
            }
            self.before = view.doc.retained_frames();
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
        let after = window.read_with(cx, |view, _| view.doc.retained_frames())?;
        let counts = delta(if mode == "cold" { (0, 0) } else { self.before }, after)?;
        if self.relayout && counts != (1, 0) {
            return Err(format!(
                "relayout requires one document build and no replay; received {counts:?}"
            )
            .into());
        }
        let group = usize::from(self.relayout && !index.is_multiple_of(2));
        self.samples.push(json!({"mode": mode, "sample_index": index,
            "css_viewport": viewport(self.base, group == 1), "builds": counts.0, "replays": counts.1}));
        self.pixels[group].observe(frame.pixels, frame.seq, mode, index)
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
            let different = differing(incremental, &fresh.pixels)?;
            let mut check = json!({"css_viewport": css, "different_rgb_pixels": different});
            if different != 0 {
                check["detail"] = detail(incremental, &fresh.pixels);
            }
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

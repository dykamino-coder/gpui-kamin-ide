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
        // A frame whose reused blocks were refused at prepaint is built once
        // more within the same frame; both builds are part of the sample.
        if self.relayout && (counts.0 == 0 || counts.1 != 0) {
            return Err(format!(
                "relayout requires a document build and no replay; received {counts:?}"
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
        let directory = run.3.frame_diff_dir.as_deref();
        let result = self.finish_checks(report, run).await;
        if let Err(error) = &result
            && let Err(capture) = preserve_failure(directory, report, &error.to_string())
        {
            eprintln!("failed-frame capture failed: {capture}; original refusal: {error}");
        }
        result
    }

    async fn finish_checks(self, report: &mut Value, run: Run<'_, '_>) -> Result<()> {
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

/// Первая причина отказа и уже сохранённые RGB-артефакты не теряются при retry.
fn preserve_failure(
    directory: Option<&std::path::Path>,
    report: &Value,
    error: &str,
) -> Result<()> {
    let Some(directory) = directory else {
        return Ok(());
    };
    let path = directory.join("failed-frame.json");
    let file = match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    serde_json::to_writer_pretty(file, &json!({"error": error, "report": report}))?;
    Ok(())
}

#[cfg(test)]
mod tests;

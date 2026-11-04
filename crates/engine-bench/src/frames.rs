//! CSS-вход и RGB-последовательности измерений; replay отличим от реальной раскладки.

use super::{
    args::{Args, Mode},
    journal::Journal,
    pixels::Recorder,
    view::View,
};
use gpui::{AsyncApp, WindowHandle};
use kamin_html::host::capture::Frame;
use serde_json::{Value, json};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

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
    samples: Journal,
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
            samples: Journal::new()?,
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
        self.record(frame, mode, index, counts)
    }

    pub(super) fn record(
        &mut self,
        frame: Frame,
        mode: &str,
        index: usize,
        counts: (u64, u64),
    ) -> Result<()> {
        if self.relayout && counts != (1, 0) {
            return Err(format!(
                "relayout requires one document build and no replay; received {counts:?}"
            )
            .into());
        }
        let group = usize::from(self.relayout && !index.is_multiple_of(2));
        self.samples.push(&json!({"mode": mode, "sample_index": index,
            "css_viewport": viewport(self.base, group == 1), "builds": counts.0, "replays": counts.1}))?;
        self.pixels[group].observe(frame.pixels, frame.seq, mode, index)
    }

    pub(super) fn release_baselines(&mut self) {
        for pixels in &mut self.pixels {
            pixels.release_baseline();
        }
    }

    pub(super) fn changed(&self) -> bool {
        self.pixels.iter().any(Recorder::changed)
    }

    pub(super) fn report(&mut self) -> Result<Value> {
        let mut out = if self.relayout {
            let mut sequences = Vec::new();
            for (group, pixels) in self.pixels.iter_mut().enumerate() {
                sequences.push(json!({"css_viewport": viewport(self.base, group == 1), "rgb": pixels.report()?}));
            }
            json!({"frame_pixels_changed": self.changed(),
                "frame_pixel_sequences": sequences,
                "relayout_trigger": "alternating RenderOpts CSS viewport on the same Document; fixed capture surface",
                "relayout_viewport_sequence": [viewport(self.base, true), self.base]})
        } else {
            self.pixels[0].report()?
        };
        out["document_frame_samples"] = Value::Array(self.samples.read()?);
        out["warm_scope"] = json!("unchanged document: may replay without element build/layout");
        Ok(out)
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

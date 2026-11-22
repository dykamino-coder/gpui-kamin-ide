//! Fresh-document validation and memory reporting after measured frames.
use super::{Frames, Result, Run, viewport};
use crate::run::{Step, sample};
use serde_json::{Value, json};

impl Frames {
    /// Validate relayout against a fresh document and audit steady memory,
    /// after reading peak memory and outside measured intervals.
    pub(crate) async fn finish(self, report: &mut Value, run: Run<'_, '_>) -> Result<()> {
        let (cx, window, fonts, args, capture, luid) = run;
        self.fresh(report, (&mut *cx, window, fonts, args, capture, luid))
            .await?;
        if args.memory_settle_ms > 0 {
            if report["frame_pixels_changed"] != false {
                return Err("memory audit requires an unchanged RGB frame sequence".into());
            }
            report["memory_audit"] =
                crate::memory::finish(cx, window, fonts, args, capture, luid).await?;
        }
        Ok(())
    }

    /// Compare the first captured frame of each width with a fresh document
    /// at that width and animation time. Later changes set frame_pixels_changed.
    async fn fresh(self, report: &mut Value, run: Run<'_, '_>) -> Result<()> {
        if self.scroll && super::memory_only() {
            report["scroll_fresh_check"] = json!("skipped: ENGINE_BENCH_SCROLL_MEMORY_ONLY");
            return Ok(());
        }
        if self.scroll {
            return crate::scroll_fresh::check(self.steps, report, run).await;
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
            super::super::animation::prepare(cx, window, args, if group == 1 { 1 } else { 2 })?;
            window.update(cx, |view, _, _| view.viewport = Some(css))?;
            let (_, fresh, _) =
                sample(cx, window, fonts, args, Step::Cold(None), capture, luid).await?;
            let mut check = crate::fresh_check::compare(
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

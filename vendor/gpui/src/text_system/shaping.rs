//! Caller-local shaping policy; browser text retains shaping across inline boxes.

use super::{FontRun, TextRun, WindowTextSystem};
use crate::Pixels;

impl WindowTextSystem {
    /// Return a caller-local handle with the chosen style-boundary shaping policy.
    ///
    /// Normal GPUI text breaks ligatures between differently styled runs. Browser
    /// callers should pass `false` for CSS Text boundary shaping. This handle
    /// shares the window's cache without changing the policy of other callers.
    pub fn with_ligature_breaking(&self, enabled: bool) -> Self {
        Self {
            break_ligatures: enabled,
            ..self.clone()
        }
    }

    pub(super) fn fill_font_runs(
        &self,
        runs: &[TextRun],
        font_size: Pixels,
        font_runs: &mut Vec<FontRun>,
    ) {
        for (index, run) in runs.iter().enumerate() {
            let font_id = self.resolve_font(&run.font);
            let run_size = run.font_size.unwrap_or(font_size);
            let same_style = index > 0 && {
                let previous = &runs[index - 1];
                previous.color == run.color
                    && previous.underline == run.underline
                    && previous.strikethrough == run.strikethrough
                    && previous.background_color == run.background_color
            };
            if let Some(font_run) = font_runs.last_mut()
                && font_run.font_id == font_id
                && font_run.font_size == run_size
                && (!self.break_ligatures || same_style)
            {
                font_run.len += run.len;
            } else {
                font_runs.push(FontRun {
                    len: run.len,
                    font_id,
                    font_size: run_size,
                    break_ligatures: self.break_ligatures,
                });
            }
        }
    }
}

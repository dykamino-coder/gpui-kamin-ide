//! The line fragments of a painted paragraph's inline boxes, kept with its
//! container box (`layout::retained::fragments`): every line, culled lines
//! too, names the band owners with a run on it.

use super::cull::Cull;
use crate::text::paragraph::*;
use gpui::{Pixels, Window};

impl Paragraph {
    /// Write the fragments of this paint, its lines placed at `bands`. Only
    /// a paragraph whose lines are culled is placed in window space alone;
    /// another writes nothing and its container keeps an older stamp.
    pub(super) fn write_fragments(
        &self,
        cull: Result<&Cull, &'static str>,
        bands: &[(Pixels, Pixels)],
        window: &Window,
    ) {
        let Some(target) = self.fragments.as_ref() else {
            return;
        };
        let cull = match cull {
            Ok(cull) => cull,
            Err(reason) => {
                target.store.fragment_refused(target.owner, reason);
                return;
            }
        };
        // Text of no inline box has no fragment: its container keeps no slot.
        if self.runs.iter().all(|r| r.background_owner == 0) {
            target
                .store
                .fragment_refused(target.owner, "paragraph has no band owners");
            return;
        }
        let (extent, bleed) = cull.reach();
        target.write(extent, bleed, window.scale_factor(), |out| {
            let (runs, mut r, mut at) = (&self.runs, 0, 0);
            for (line, band) in self.lines.iter().zip(bands) {
                while r < runs.len() && at + runs[r].len <= line.range.start {
                    at += runs[r].len;
                    r += 1;
                }
                let (mut k, mut start) = (r, at);
                while k < runs.len() && start < line.range.end {
                    out.push(runs[k].background_owner, band.0.into(), band.1.into());
                    start += runs[k].len;
                    k += 1;
                }
            }
        });
    }
}

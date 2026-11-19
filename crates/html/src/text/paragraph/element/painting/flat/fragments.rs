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
        cull: Option<&Cull>,
        bands: &[(Pixels, Pixels)],
        window: &Window,
    ) {
        let (Some(target), Some(cull)) = (self.fragments.as_ref(), cull) else {
            return;
        };
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

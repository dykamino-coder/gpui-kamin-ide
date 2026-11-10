//! Caller-local shaping policy; browser text retains shaping across inline boxes.
//! KaminIDE patch: uncached shaping for callers that keep their own shaped
//! pieces (a long paragraph shaped piece by piece, `shape_into`).

use super::{FontId, FontRun, ShapedRun, TextRun, WindowTextSystem, line_layout::track_glyphs};
use super::Font;
use crate::{Pixels, px};
use std::borrow::Cow;

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
            let same_style = index > 0 && same_style(&runs[index - 1], run);
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

    /// `fill_font_runs` for the text that starts `skip` bytes into the first
    /// of `runs` and is `len` bytes long.
    fn fill_font_runs_clipped(
        &self,
        runs: &[TextRun],
        skip: usize,
        len: usize,
        font_size: Pixels,
        font_runs: &mut Vec<FontRun>,
    ) {
        let mut left = len;
        let mut previous: Option<&TextRun> = None;
        for (index, run) in runs.iter().enumerate() {
            let run_len = run
                .len
                .saturating_sub(if index == 0 { skip } else { 0 })
                .min(left);
            // Like a caller's own slice of the runs: empty pieces are left out.
            if run_len == 0 {
                continue;
            }
            left -= run_len;
            let font_id = self.resolve_font(&run.font);
            let run_size = run.font_size.unwrap_or(font_size);
            let same_style = previous.is_some_and(|previous| same_style(previous, run));
            previous = Some(run);
            if let Some(font_run) = font_runs.last_mut()
                && font_run.font_id == font_id
                && font_run.font_size == run_size
                && (!self.break_ligatures || same_style)
            {
                font_run.len += run_len;
            } else {
                font_runs.push(FontRun {
                    len: run_len,
                    font_id,
                    font_size: run_size,
                    break_ligatures: self.break_ligatures,
                });
            }
        }
    }

    /// The text a line shapes: a non-breaking hyphen (U+2011) without a glyph
    /// in its run's font takes that font's hyphen U+2010 (see
    /// `layout_line_spaced`); both are three UTF-8 bytes, so run lengths hold.
    pub(super) fn no_break_hyphens<'t>(&self, text: &'t str, font_runs: &[FontRun]) -> Cow<'t, str> {
        if !text.contains('\u{2011}') {
            return Cow::Borrowed(text);
        }
        let mut out = String::with_capacity(text.len());
        let mut runs_iter = font_runs.iter();
        let mut run = runs_iter.next();
        let mut run_end = run.map_or(usize::MAX, |r| r.len);
        for (i, ch) in text.char_indices() {
            while i >= run_end {
                run = runs_iter.next();
                let start = run_end;
                run_end = run.map_or(usize::MAX, |r| start + r.len);
            }
            // Отсутствующий знак DirectWrite отдаёт глифом 0 (.notdef), а не `None`.
            let has = |font_id: FontId, c: char| {
                self.platform_text_system
                    .glyph_for_char(font_id, c)
                    .is_some_and(|g| g.0 != 0)
            };
            let swap = ch == '\u{2011}'
                && run.is_some_and(|r| !has(r.font_id, '\u{2011}') && has(r.font_id, '\u{2010}'));
            out.push(if swap { '\u{2010}' } else { ch });
        }
        Cow::Owned(out)
    }

    /// `layout_line_spaced` without the line cache, for a caller that keeps
    /// the shaped text itself: `text` is styled by `runs` with the first
    /// `skip` bytes of the first run dropped. Its glyph runs are appended to
    /// `out`, glyph indices offset by `index_base` and positions from the
    /// start of `text`. Returns the width (letter-spacing included), ascent
    /// and descent.
    #[allow(clippy::too_many_arguments)]
    pub fn shape_into(
        &self,
        text: &str,
        font_size: Pixels,
        runs: &[TextRun],
        skip: usize,
        letter_spacing: Pixels,
        index_base: usize,
        out: &mut Vec<ShapedRun>,
    ) -> (Pixels, Pixels, Pixels) {
        let mut font_runs = self.font_runs_pool.lock().pop().unwrap_or_default();
        font_runs.clear();
        self.fill_font_runs_clipped(runs, skip, text.len(), font_size, &mut font_runs);
        let shaped = self.no_break_hyphens(text, &font_runs);
        let first = out.len();
        let (mut width, ascent, descent) = self.platform_text_system.layout_line_into(
            &shaped,
            font_size,
            &font_runs,
            index_base,
            out,
        );
        self.font_runs_pool.lock().push(font_runs);
        if letter_spacing != px(0.) {
            width += track_glyphs(&mut out[first..], &shaped, index_base, letter_spacing);
        }
        (width, ascent, descent)
    }

    /// Whether a line shapes text in `a` (at size `a_size`) and text in `b`
    /// apart: no kerning, contextual form or ligature joins glyphs across a
    /// boundary between them.
    pub fn shapes_apart(&self, a: &Font, a_size: Pixels, b: &Font, b_size: Pixels) -> bool {
        let (a, b) = (self.resolve_font(a), self.resolve_font(b));
        self.platform_text_system
            .shapes_apart((a, a_size), (b, b_size))
    }
}

/// Neighbouring runs drawn alike (a ligature across them is kept when breaking is on).
fn same_style(previous: &TextRun, run: &TextRun) -> bool {
    previous.color == run.color
        && previous.underline == run.underline
        && previous.strikethrough == run.strikethrough
        && previous.background_color == run.background_color
}

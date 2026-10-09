//! Shape text-overflow strings as bidi isolates at the paragraph's direction.

use super::{Paragraph, controlled_shape, slice_runs};
use gpui::{App, Pixels, Point, ShapedLine, SharedString, Window, point};
use unicode_bidi::{BidiInfo, Level};

impl Paragraph {
    pub(super) fn shaped_suffix(
        &self,
        mark: &str,
        at: usize,
        window: &mut Window,
    ) -> Vec<ShapedLine> {
        if mark.is_empty() {
            return Vec::new();
        }
        let mut runs = slice_runs(&self.runs, &(at..at + 1));
        let Some(run) = runs.first_mut() else {
            return Vec::new();
        };
        run.len = mark.len();
        self.style_marker_run(mark, run);
        let run = run.clone();
        if self.is_block_mark(mark) || self.overflow_marker.as_deref() != Some(mark) {
            return vec![window.text_system().with_ligature_breaking(false).shape_line_spaced(
                SharedString::from(mark.to_string()),
                self.font_size,
                &[run],
                None,
                self.letter_spacing,
            )];
        }
        // CSS Overflow 4 §5.1: the anonymous isolate inherits the bidi
        // paragraph's direction; its first strong character cannot choose it.
        let base = if self.plaintext.is_some() {
            BidiInfo::new(&self.text, None)
                .paragraphs
                .iter()
                .find(|paragraph| paragraph.range.contains(&at))
                .map_or(Level::ltr(), |paragraph| paragraph.level)
        } else if self.wrap.rtl {
            Level::rtl()
        } else {
            Level::ltr()
        };
        let info = BidiInfo::new(mark, Some(base));
        let Some(paragraph) = info.paragraphs.first() else {
            return Vec::new();
        };
        let (levels, visual) = info.visual_runs(paragraph, 0..mark.len());
        if visual.len() == 1 && !levels[visual[0].start].is_rtl() {
            return vec![window.text_system().with_ligature_breaking(false).shape_line_spaced(
                SharedString::from(mark.to_string()),
                self.font_size,
                &[run],
                None,
                self.letter_spacing,
            )];
        }
        visual
            .into_iter()
            .map(|range| {
                let body = &mark[range.clone()];
                let mut run = run.clone();
                run.len = body.len();
                if levels[range.start].is_rtl() {
                    let mut runs = [run];
                    let text = controlled_shape::text(body, &mut runs, true);
                    window.text_system().with_ligature_breaking(false).shape_line_rtl(
                        text,
                        self.font_size,
                        &runs,
                        self.letter_spacing,
                    )
                } else {
                    let mut runs = [run];
                    let text = controlled_shape::text(body, &mut runs, false);
                    window.text_system().with_ligature_breaking(false).shape_line_spaced(
                        text,
                        self.font_size,
                        &runs,
                        None,
                        self.letter_spacing,
                    )
                }
            })
            .collect()
    }
    pub(super) fn paint_suffix(
        &self,
        mark: &str,
        at: usize,
        mut origin: Point<Pixels>,
        base: Option<Pixels>,
        raw: bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        for shaped in self.shaped_suffix(mark, at, window) {
            // Each visual piece is an inline run on the same line baseline.
            let aligned = match base {
                Some(base) => {
                    let own =
                        (self.line_height - shaped.ascent - shaped.descent) / 2.0 + shaped.ascent;
                    point(origin.x, origin.y + base - own)
                }
                None => origin,
            };
            // Keep the line's exact Ahem placement. Antialiased marker runs
            // use the paragraph's shared baseline rasterization, like text.
            let paint_origin = if raw
                && shaped
                    .runs
                    .iter()
                    .all(|run| window.text_system().pixel_exact_glyphs(run.font_id))
            {
                aligned
            } else {
                self.text_raster_origin(&shaped, aligned, window)
            };
            let _ = shaped.paint(paint_origin, self.line_height, gpui::TextAlign::Left, None, window, cx);
            origin.x += shaped.width;
        }
    }
}

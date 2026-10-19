//! Paragraph construction and explicit document measurement and ruby inputs.

use super::Paragraph;
use super::{Align, Indent, Wrap};
use crate::text::paragraph::tabs;
use gpui::{Hsla, Pixels, SharedString, TextRun, point, px};

impl Paragraph {
    pub fn new(
        text: SharedString,
        runs: Vec<TextRun>,
        font_size: Pixels,
        line_height: Pixels,
        align: Align,
        wrap: Wrap,
    ) -> Self {
        Paragraph {
            paint_context: Default::default(),
            clamp_rows: Default::default(),
            measures: Default::default(),
            scroll_owner: None,
            ruby_base_sink: None,
            ruby_layers: None,
            text,
            runs,
            font_size,
            line_height,
            align,
            align_last: None,
            ruby_justify: false,
            justify_chars: 1,
            ruby_unit: false,
            ruby_rotated: false,
            plaintext: None,
            lines_reversed: false,
            letter_spacing: px(0.),
            word_spacing: px(0.),
            vertical: false,
            vertical_rl: false,
            vertical_central_baseline: true,
            rotated_central: false,
            vertical_ccw: false,
            selection_vertical: None,
            vertical_layout_origin: point(px(0.0), px(0.0)),
            glyph_nudge: point(px(0.0), px(0.0)),
            opaque_text_origin: false,
            width_nudge: px(0.0),
            indent_basis: None,
            vertical_inline: None,
            ortho_limit: None,
            hanging: crate::style::computed::Hanging::default(),
            indent: Indent::default(),
            spacers: Vec::new(),
            spacer_edges: Vec::new(),
            box_extents: Vec::new(),
            flow: std::sync::Arc::new((Vec::new(), Vec::new())),
            id: None,
            highlight: Hsla::default(),
            wrap,
            spans: Vec::new(),
            word_spans: Vec::new(),
            letter_spans: Vec::new(),
            shift_spans: Vec::new(),
            lh_spans: Vec::new(),
            rel_spans: Vec::new(),
            atoms: Vec::new(),
            atom_boxes: Vec::new(),
            atom_fit: Default::default(),
            strut: (0.0, 0.0, 0.0),
            run_metrics: Vec::new(),
            edge_spans: Vec::new(),
            ruby_trim: Default::default(),
            emph_spans: Vec::new(),
            decor_spans: Vec::new(),
            decor_pending: Default::default(),
            box_spans: Vec::new(),
            strut_run: None,
            strut_box: (0.0, 0.0),
            first_strut: None,
            lines: Vec::new(),
            fragment: None,
            clamp: None,
            clamp_force: false,
            text_overflow: false,
            overflow_marker: None,
            clamp_marker: None,
            clamp_tag: None,
            unbalanced_steps: None,
            marker_font: None,
            marker_size: None,
            marker_color: None,
            fit: None,
            fit_spacing_scalable: true,
            fit_line_height_fixed: false,
            tab_stop: tabs::TabStops::uniform(64.0),
            hyphen: SharedString::from("\u{2010}"),
            hyphen_limit_lines: usize::MAX,
            hyphen_w: std::cell::RefCell::new(Vec::new()),
            overlays: Vec::new(),
            overlay_hidden: Vec::new(),
        }
    }
}

impl Paragraph {
    /// Measurement owner shared with other paragraphs in this document.
    pub(crate) fn measure_context(mut self, cx: &crate::layout::cx::LayoutCx) -> Self {
        self.measures = cx.doc().paragraph_measures.clone();
        self.paint_context = cx.vertical_paint.clone();
        self.clamp_rows = cx.doc().paragraph_rows.clone();
        self
    }

    /// Ruby context of the place where the paragraph is built: the base-width
    /// sink of the ruby column (the first paragraph built under it takes it)
    /// and the base's annotation layers.
    pub(crate) fn ruby_context(mut self, cx: &crate::layout::cx::LayoutCx) -> Self {
        self.ruby_base_sink = cx.ruby_base.as_ref().and_then(|offer| offer.take());
        self.ruby_layers = cx.ruby_layers.clone();
        self
    }

    /// Пустой абзац — только чтобы на миг занять место настоящего, пока тот
    /// рисуется в повёрнутой системе координат.
    pub(crate) fn empty() -> Self {
        Paragraph::new(
            SharedString::default(),
            Vec::new(),
            px(0.),
            px(0.),
            Align::Left,
            Wrap::default(),
        )
    }
}

//! Выделение абзаца: снимок раскладки и поиск символа под мышью.

use crate::interactive::selection::{selected_runs, track_selection};
use crate::text::paragraph::*;
use gpui::{Bounds, GlobalElementId, Hitbox, Pixels, Point, TextRun, Window};

impl Paragraph {
    /// Прогоны с подложкой на выделенном куске: прогон нельзя раскрасить
    /// наполовину, поэтому попавшие на границу режутся надвое.
    pub(crate) fn runs_with_selection(&self, from: usize, to: usize) -> Vec<TextRun> {
        selected_runs(&self.runs, self.highlight, from, to)
    }
}

impl Paragraph {
    /// Тянуть выделение мышью.
    pub(crate) fn track_selection(
        &self,
        global: &GlobalElementId,
        segs: &[Seg],
        bounds: Bounds<Pixels>,
        hitbox: Hitbox,
        window: &mut Window,
    ) {
        let inside = hitbox.is_hovered(window);
        if inside {
            window.set_cursor_style(gpui::CursorStyle::IBeam, &hitbox);
        }
        // Замыкания живут дольше кадра, поэтому берут СВОЙ снимок раскладки.
        let probe = Paragraph {
            measures: self.measures.clone(),
            scroll_owner: None,
            plaintext: self.plaintext,
            lines_reversed: self.lines_reversed,
            flow: self.flow.clone(),
            text: self.text.clone(),
            spans: self.spans.clone(),
            word_spans: self.word_spans.clone(),
            letter_spans: self.letter_spans.clone(),
            shift_spans: self.shift_spans.clone(),
            lh_spans: self.lh_spans.clone(),
            rel_spans: self.rel_spans.clone(),
            atoms: Vec::new(),
            atom_boxes: self.atom_boxes.clone(),
            atom_fit: Default::default(),
            strut: self.strut,
            run_metrics: self.run_metrics.clone(),
            edge_spans: self.edge_spans.clone(),
            ruby_trim: self.ruby_trim,
            emph_spans: self.emph_spans.clone(),
            decor_spans: Vec::new(),
            decor_pending: Default::default(),
            box_spans: self.box_spans.clone(),
            strut_run: None,
            strut_box: self.strut_box,
            first_strut: self.first_strut.clone(),
            ortho_limit: self.ortho_limit,
            runs: Vec::new(),
            font_size: self.font_size,
            line_height: self.line_height,
            align: self.align,
            align_last: self.align_last,
            ruby_justify: self.ruby_justify,
            justify_chars: self.justify_chars,
            ruby_unit: self.ruby_unit,
            ruby_rotated: self.ruby_rotated,
            ruby_base_sink: None,
            ruby_layers: None,
            letter_spacing: self.letter_spacing,
            word_spacing: self.word_spacing,
            vertical: self.vertical,
            vertical_rl: self.vertical_rl,
            vertical_central_baseline: self.vertical_central_baseline,
            rotated_central: self.rotated_central,
            vertical_ccw: self.vertical_ccw,
            selection_vertical: self.selection_vertical,
            vertical_layout_origin: self.vertical_layout_origin,
            glyph_nudge: self.glyph_nudge,
            opaque_text_origin: self.opaque_text_origin,
            width_nudge: self.width_nudge,
            indent_basis: self.indent_basis,
            vertical_inline: self.vertical_inline,
            hanging: self.hanging,
            indent: self.indent,
            spacers: self.spacers.clone(),
            spacer_edges: self.spacer_edges.clone(),
            box_extents: self.box_extents.clone(),
            id: None,
            highlight: self.highlight,
            wrap: self.wrap,
            lines: self.lines.clone(),
            clamp: self.clamp,
            clamp_force: self.clamp_force,
            fit_spacing_scalable: self.fit_spacing_scalable,
            fit_line_height_fixed: self.fit_line_height_fixed,
            text_overflow: false,
            overflow_marker: None,
            clamp_marker: None,
            clamp_tag: None,
            unbalanced_steps: None,
            marker_font: None,
            marker_size: None,
            marker_color: None,
            fit: self.fit,
            tab_stop: self.tab_stop.clone(),
            hyphen: self.hyphen.clone(),
            hyphen_limit_lines: self.hyphen_limit_lines,
            hyphen_w: std::cell::RefCell::new(self.hyphen_w.borrow().clone()),
            overlays: Vec::new(),
            overlay_hidden: Vec::new(),
        };
        let segs = segs.to_vec();
        let probe = std::rc::Rc::new(probe);
        let segs = std::rc::Rc::new(segs);
        track_selection(
            global,
            inside,
            move |p: Point<Pixels>| Some(probe.index_at(&segs, bounds, p)),
            window,
        );
    }
}

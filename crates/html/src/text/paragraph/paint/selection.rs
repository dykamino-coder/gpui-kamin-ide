//! Selection for paint; split out to keep the owning module within 250 lines.

use super::Selection;
use crate::text::paragraph::*;
use gpui::{
    Bounds, GlobalElementId, Hitbox, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    Pixels, Point, TextRun, Window,
};

impl Paragraph {
    /// Прогоны с подложкой на выделенном куске: прогон нельзя раскрасить
    /// наполовину, поэтому попавшие на границу режутся надвое.
    pub(crate) fn runs_with_selection(&self, from: usize, to: usize) -> Vec<TextRun> {
        if from >= to {
            return self.runs.clone();
        }
        let mut out = Vec::with_capacity(self.runs.len() + 2);
        let mut at = 0usize;
        for run in &self.runs {
            let end = at + run.len;
            let mut cut = |start: usize, stop: usize, selected: bool| {
                if stop <= start {
                    return;
                }
                let mut piece = run.clone();
                piece.len = stop - start;
                piece.background_color = selected.then_some(self.highlight);
                out.push(piece);
            };
            cut(at, end.min(from), false);
            cut(at.max(from), end.min(to), true);
            cut(at.max(to), end, false);
            at = end;
        }
        out
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
            ortho_limit: self.ortho_limit,
            runs: Vec::new(),
            font_size: self.font_size,
            line_height: self.line_height,
            align: self.align,
            align_last: self.align_last,
            ruby_justify: self.ruby_justify,
            justify_chars: self.justify_chars,
            ruby_unit: self.ruby_unit,
            ruby_base_sink: None,
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
            hyphen_w: std::cell::RefCell::new(self.hyphen_w.borrow().clone()),
            overlays: Vec::new(),
        };
        let segs = segs.to_vec();
        window.with_element_state::<Selection, _>(global, |state, window| {
            let st = std::rc::Rc::new(std::cell::Cell::new(state.unwrap_or_default()));
            let index_at = {
                let probe = std::rc::Rc::new(probe);
                let segs = std::rc::Rc::new(segs);
                move |p: Point<Pixels>| probe.index_at(&segs, bounds, p)
            };

            let down = st.clone();
            let at_down = index_at.clone();
            window.on_mouse_event(move |e: &MouseDownEvent, phase, window, _cx| {
                if !phase.bubble() || e.button != MouseButton::Left || !inside {
                    return;
                }
                let i = at_down(e.position);
                down.set(Selection {
                    anchor: i,
                    head: i,
                    dragging: true,
                });
                window.refresh();
            });

            let mv = st.clone();
            let at_move = index_at.clone();
            window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, _cx| {
                if !phase.bubble() {
                    return;
                }
                let mut s = mv.get();
                if !s.dragging {
                    return;
                }
                let i = at_move(e.position);
                if s.head != i {
                    s.head = i;
                    mv.set(s);
                    window.refresh();
                }
            });

            let up = st.clone();
            window.on_mouse_event(move |_e: &MouseUpEvent, phase, _window, _cx| {
                if !phase.bubble() {
                    return;
                }
                let mut s = up.get();
                if s.dragging {
                    s.dragging = false;
                    up.set(s);
                }
            });
            ((), st.get())
        });
    }
}

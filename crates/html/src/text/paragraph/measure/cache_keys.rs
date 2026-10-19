//! Measurement and line-breaking cache identities.

use crate::text::paragraph::*;
use gpui::Pixels;

impl Paragraph {
    /// Ключ памяти замера: от чего зависит положение знаков.
    pub(crate) fn measure_key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.text.hash(&mut h);
        f32::from(self.font_size).to_bits().hash(&mut h);
        f32::from(self.letter_spacing).to_bits().hash(&mut h);
        f32::from(self.word_spacing).to_bits().hash(&mut h);
        // Joined text is shaped without scalar GPUI tracking, then spaced by
        // typographic units. The visual word path still uses scalar tracking.
        self.joined_spacing().hash(&mut h);
        self.tab_stop.hash_into(&mut h);
        for run in &self.runs {
            run.len.hash(&mut h);
            run.font_size.map(|s| f32::from(s).to_bits()).hash(&mut h);
            run.font.family.hash(&mut h);
            run.font.weight.0.to_bits().hash(&mut h);
            (run.font.style as u8).hash(&mut h);
            // Возможности OpenType меняют и подстановку, и продвижение
            // (`vert`, `hwid`) — без них кэш отдавал чужой набор.
            for (tag, value) in run.font.features.tag_value_list() {
                tag.hash(&mut h);
                value.hash(&mut h);
            }
        }
        // Segments after a tab (or a soft hyphen) retain cumulative offsets.
        // Those offsets include inline spacing and ruby spacer advances;
        // changing overhang must not reuse the tab stops from the wider ruby.
        for (range, spacing) in self.word_spans.iter().chain(&self.letter_spans) {
            range.start.hash(&mut h);
            range.end.hash(&mut h);
            f32::from(*spacing).to_bits().hash(&mut h);
        }
        h.finish()
    }
}

impl Paragraph {
    /// Ключ РАЗРЕЗА: замер плюс всё, от чего зависит перенос и ширины строк.
    pub(crate) fn split_key(&self, limit: Option<Pixels>) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.measure_key().hash(&mut h);
        limit.map(|l| f32::from(l).to_bits()).hash(&mut h);
        self.indent_basis
            .map(|l| f32::from(l).to_bits())
            .hash(&mut h);
        let w = &self.wrap;
        [
            w.nowrap,
            w.break_spaces,
            w.break_all,
            w.anywhere,
            w.keep_all,
            w.break_word,
            w.wrap_anywhere,
            w.rtl,
            w.balance,
            w.keep_spaces,
        ]
        .hash(&mut h);
        // Ruby sub-lines retain trailing spaces and use different expansion
        // opportunities; a normal line must not reuse their cached cut.
        [self.ruby_unit, self.ruby_justify].hash(&mut h);
        // Per-piece wrap rules (`white-space`/`word-break` of a nested inline
        // or of a `display: contents` element) change the breaks too: without
        // them a `nowrap` run reused the cached lines of an identical text
        // laid out under `normal` (`white-space-applies-to-text-001`).
        format!("{:?}{:?}", self.wrap, self.spans).hash(&mut h);
        self.indent.px.to_bits().hash(&mut h);
        for f in self.flow.0.iter().chain(self.flow.1.iter()) {
            f.hash_bits().hash(&mut h);
        }
        if !self.flow.0.is_empty() || !self.flow.1.is_empty() {
            // Float opportunities depend on the content's block extent as well as width.
            format!(
                "{:?}",
                (
                    self.line_height,
                    &self.atom_boxes,
                    &self.shift_spans,
                    &self.lh_spans,
                    &self.edge_spans,
                    &self.box_spans,
                    &self.emph_spans,
                    self.ruby_trim,
                    self.strut_box,
                    self.strut,
                    &self.run_metrics
                )
            )
            .hash(&mut h);
        }
        self.indent.pct.to_bits().hash(&mut h);
        self.indent.each_line.hash(&mut h);
        self.indent.hanging.hash(&mut h);
        let hg = &self.hanging;
        [hg.first, hg.last, hg.force_end, hg.allow_end].hash(&mut h);
        self.clamp.hash(&mut h);
        self.clamp_force.hash(&mut h);
        self.text_overflow.hash(&mut h);
        self.overflow_marker.hash(&mut h);
        self.clamp_marker.hash(&mut h);
        self.marker_size
            .map(|s| f32::from(s).to_bits())
            .hash(&mut h);
        self.hyphen.hash(&mut h);
        self.hyphen_limit_lines.hash(&mut h);
        self.spacers.hash(&mut h);
        for (r, v) in self.word_spans.iter().chain(&self.letter_spans) {
            r.start.hash(&mut h);
            r.end.hash(&mut h);
            f32::from(*v).to_bits().hash(&mut h);
        }
        h.finish()
    }
}

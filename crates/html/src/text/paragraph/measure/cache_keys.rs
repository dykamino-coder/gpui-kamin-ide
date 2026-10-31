//! Ключи памяти замера и разреза: от чего зависит положение знаков (текст,
//! кегль, трекинг, шрифты и возможности OpenType прогонов, позиции
//! табуляции, интервалы по кускам) и, сверх того, от чего зависят перенос и
//! ширины строк (предел, правила переноса абзаца и кусков, отступ, вырезы
//! обтекания, свисание, обрыв, знаки обрыва и переноса, распорки). Ключ
//! обязан покрывать ВСЁ, что читает `split_uncached`, иначе устаревшие
//! переносы сдвинут пиксели.

use crate::text::paragraph::*;
use gpui::Pixels;

impl Paragraph {
    /// Ключ памяти замера: от чего зависит положение знаков.
    pub(crate) fn measure_key(&self) -> u64 {
        *self
            .prepared()
            .measure_key
            .get_or_init(|| self.measure_key_uncached())
    }

    fn measure_key_uncached(&self) -> u64 {
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
        self.runs.len().hash(&mut h);
        for run in &self.runs {
            run.len.hash(&mut h);
            run.font_size.map(|s| f32::from(s).to_bits()).hash(&mut h);
            run.font.family.hash(&mut h);
            run.font.weight.0.to_bits().hash(&mut h);
            (run.font.style as u8).hash(&mut h);
            // Возможности OpenType меняют и подстановку, и продвижение
            // (`vert`, `hwid`) — без них кэш отдавал чужой набор.
            let features = run.font.features.tag_value_list();
            features.len().hash(&mut h);
            for (tag, value) in features {
                tag.hash(&mut h);
                value.hash(&mut h);
            }
        }
        // Segments after a tab (or a soft hyphen) retain cumulative offsets.
        // Those offsets include inline spacing and ruby spacer advances;
        // changing overhang must not reuse the tab stops from the wider ruby.
        for spans in [&self.word_spans, &self.letter_spans] {
            spans.len().hash(&mut h);
            for (range, spacing) in spans {
                range.hash(&mut h);
                f32::from(*spacing).to_bits().hash(&mut h);
            }
        }
        h.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::px;

    #[test]
    fn invariant_keys_are_reused_but_constraints_and_revisions_change_them() {
        let p = Paragraph::new(
            "a b".into(),
            vec![],
            px(16.),
            px(20.),
            Align::Left,
            Wrap::default(),
        );
        let measured = p.measure_key();
        let split = p.split_key(Some(px(40.)));
        assert_eq!(p.prepared().measure_key.get(), Some(&measured));
        assert_eq!(p.split_key(Some(px(40.))), split);
        assert_ne!(p.split_key(Some(px(41.))), split);
        let p = p.letter_spans(vec![(0..3, px(0.1))]);
        assert_ne!(p.measure_key(), measured);
        assert_ne!(p.split_key(Some(px(40.))), split);
        let mut p = p;
        let split = p.split_key(Some(px(40.)));
        p.indent_basis = Some(px(200.));
        assert_ne!(p.split_key(Some(px(40.))), split);
    }

    #[test]
    fn typed_wrap_and_float_geometry_keys_distinguish_break_inputs() {
        use crate::layout::float::shapes::FloatShape;
        let mut p = Paragraph::new(
            "a b".into(),
            vec![],
            px(16.),
            px(20.),
            Align::Left,
            Wrap::default(),
        );
        p.flow = std::sync::Arc::new((
            vec![FloatShape::Band {
                top: 0.,
                h: 80.,
                w: 20.,
            }],
            vec![],
        ));
        let key = p.split_key(None);
        p.wrap.break_spaces = true;
        p.invalidate_prepared();
        assert_ne!(p.split_key(None), key);
        let key = p.split_key(None);
        let p = p.shift_spans(vec![(0..3, px(0.125))]);
        assert_ne!(p.split_key(None), key);
        let key = p.split_key(None);
        let p = p.shift_spans(vec![(0..3, px(0.126))]);
        assert_ne!(p.split_key(None), key);
        let key = p.split_key(None);
        let swapped = std::sync::Arc::new((p.flow.1.clone(), p.flow.0.clone()));
        let p = p.flow_shapes(swapped);
        assert_ne!(p.split_key(None), key);
    }

    #[test]
    fn word_and_letter_spans_have_distinct_cache_namespaces() {
        let p = || {
            Paragraph::new(
                "a b".into(),
                vec![],
                gpui::px(16.),
                gpui::px(20.),
                Align::Justify,
                Wrap::default(),
            )
        };
        let words = p().word_spans(vec![(0..3, gpui::px(0.5))]);
        let letters = p().letter_spans(vec![(0..3, gpui::px(0.5))]);
        assert_ne!(words.measure_key(), letters.measure_key());
        assert_ne!(words.split_key(None), letters.split_key(None));
    }
}

impl Paragraph {
    /// Ключ РАЗРЕЗА: замер плюс всё, от чего зависит перенос и ширины строк.
    pub(crate) fn split_key(&self, limit: Option<Pixels>) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.prepared()
            .split_key
            .get_or_init(|| self.split_key_uncached())
            .hash(&mut h);
        limit.map(|l| f32::from(l).to_bits()).hash(&mut h);
        self.indent_basis
            .map(|l| f32::from(l).to_bits())
            .hash(&mut h);
        h.finish()
    }

    fn split_key_uncached(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.measure_key().hash(&mut h);
        self.wrap.hash(&mut h);
        // Ruby sub-lines retain trailing spaces and use different expansion
        // opportunities; a normal line must not reuse their cached cut.
        [self.ruby_unit, self.ruby_justify].hash(&mut h);
        // Per-piece wrap rules (`white-space`/`word-break` of a nested inline
        // or of a `display: contents` element) change the breaks too: without
        // them a `nowrap` run reused the cached lines of an identical text
        // laid out under `normal` (`white-space-applies-to-text-001`).
        self.spans.hash(&mut h);
        self.indent.px.to_bits().hash(&mut h);
        for side in [&self.flow.0, &self.flow.1] {
            side.len().hash(&mut h);
            for f in side {
                f.hash_bits().hash(&mut h);
            }
        }
        if !self.flow.0.is_empty() || !self.flow.1.is_empty() {
            self.hash_line_geometry(&mut h);
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

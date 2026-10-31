//! Координаты набранного сегмента: один проход по глифам и межсловным
//! добавкам. Трекинг сохраняет порядок f32; точная двоичная сумма ускоряет
//! только случаи, где каждое прежнее сложение доказуемо представимо.

use super::super::*;
use gpui::{Pixels, px};

#[derive(Clone, Copy)]
struct ExactSum {
    sum: f64,
    absolute: f64,
    unit: f64,
}

impl Default for ExactSum {
    fn default() -> Self {
        Self {
            sum: 0.0,
            absolute: 0.0,
            unit: f64::INFINITY,
        }
    }
}

fn quantum(value: f32) -> f64 {
    let bits = value.to_bits() & 0x7fff_ffff;
    if bits == 0 {
        return f64::INFINITY;
    }
    if !value.is_finite() {
        return 0.0;
    }
    let exponent = (bits >> 23) as i32;
    let mantissa = (bits & 0x7f_ffff) | if exponent == 0 { 0 } else { 1 << 23 };
    2f64.powi(if exponent == 0 { -149 } else { exponent - 150 } + mantissa.trailing_zeros() as i32)
}

impl ExactSum {
    fn push(&mut self, value: f32) {
        self.sum += f64::from(value);
        self.absolute += f64::from(value).abs();
        self.unit = self.unit.min(quantum(value));
    }

    fn apply(self, base: f32, deltas: &[f32]) -> f32 {
        let unit = self.unit.min(quantum(base));
        if base.is_finite()
            && self.sum.is_finite()
            && f64::from(base).abs() + self.absolute <= 16_777_216.0 * unit
        {
            (f64::from(base) + self.sum) as f32
        } else {
            deltas.iter().fold(base, |x, delta| {
                crate::work::add!(TextPositionSteps, 1);
                x + delta
            })
        }
    }
}

impl Paragraph {
    pub(in crate::text::paragraph) fn segment_positions(&self, seg: &Seg) -> Vec<Pixels> {
        let len = seg.end - seg.start;
        crate::work::add!(TextPositionBuildBytes, len + 1);
        let mut positions = vec![seg.layout.width; len + 1];
        // x_for_index берёт ПЕРВЫЙ глиф с индексом >= байта, даже когда
        // индексы повторяются или убывают в визуальном порядке.
        let mut filled = 0;
        for glyph in seg.layout.runs.iter().flat_map(|r| &r.glyphs) {
            crate::work::add!(TextPositionSteps, 1);
            let end = glyph.index.min(len);
            if filled <= end {
                positions[filled..=end].fill(glyph.position.x);
                filled = end + 1;
            }
        }
        positions[0] = px(0.);
        positions[len] = seg.layout.width;
        if self.joined_spacing() {
            let prefix = self.spacing_prefix(seg.start, seg.end);
            let mut unit = 0;
            for (at, value) in positions.iter_mut().enumerate() {
                crate::work::add!(TextPositionSteps, 1);
                while unit + 1 < prefix.len() && prefix[unit + 1].0 <= seg.start + at {
                    unit += 1;
                }
                *value = (*value + seg.offset) + prefix[unit].1;
            }
            return positions;
        }
        let mut deltas = Vec::new();
        let mut sum = ExactSum::default();
        let mut words = px(0.);
        for (off, ch) in self.text[seg.start..seg.end].char_indices() {
            crate::work::add!(TextPositionSteps, 1);
            let at = seg.start + off;
            let base = f32::from(positions[off] + seg.offset);
            positions[off] = px(sum.apply(base, &deltas)) + words;
            if let Some(i) = self.prepared().letters.first(at) {
                let delta = f32::from(self.letter_spans[i].1 - self.letter_spacing);
                deltas.push(delta);
                sum.push(delta);
            }
            if word_separator(ch) {
                words += self
                    .prepared()
                    .words
                    .first(at)
                    .map_or(self.word_spacing, |i| self.word_spans[i].1);
            }
        }
        positions[len] = px(sum.apply(f32::from(seg.layout.width + seg.offset), &deltas)) + words;
        positions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_addition_preserves_sequential_rounding() {
        for deltas in [
            vec![0.25; 128],
            vec![0.1; 128],
            vec![16_777_216., 1., -16_777_216.],
            vec![f32::MIN_POSITIVE, -1.0],
        ] {
            for base in [0.0, -0.0, 15.125, 16_383.999, 16_777_216., -100.1] {
                let mut sum = ExactSum::default();
                for end in 0..=deltas.len() {
                    let expected = deltas[..end].iter().fold(base, |x, delta| x + delta);
                    assert_eq!(sum.apply(base, &deltas[..end]), expected);
                    if end < deltas.len() {
                        sum.push(deltas[end]);
                    }
                }
            }
        }
    }

    #[test]
    fn prepared_positions_match_original_folds_and_nonmonotone_glyphs() {
        use gpui::{LineLayout, ShapedGlyph, ShapedRun, point};
        let p = Paragraph::new(
            "a bcd".into(),
            vec![],
            px(16.),
            px(20.),
            Align::Justify,
            Wrap::default(),
        )
        .letter_spans(vec![(0..5, px(0.1)), (0..2, px(4.))])
        .word_spacing(px(0.3));
        let seg = Seg {
            start: 0,
            end: 5,
            offset: px(0.7),
            positions: Default::default(),
            layout: std::sync::Arc::new(LineLayout {
                width: px(30.),
                len: 5,
                runs: vec![ShapedRun {
                    font_id: gpui::FontId(0),
                    font_size: px(16.),
                    glyphs: [0, 3, 2, 4]
                        .into_iter()
                        .enumerate()
                        .map(|(n, index)| ShapedGlyph {
                            id: gpui::GlyphId(0),
                            index,
                            position: point(px(n as f32 * 6.), px(0.)),
                            is_emoji: false,
                        })
                        .collect(),
                }],
                ..Default::default()
            }),
        };
        let positions = p.segment_positions(&seg);
        for (i, &actual) in positions.iter().enumerate() {
            let mut expected = if i == 0 {
                px(0.)
            } else if i == 5 {
                seg.layout.width
            } else {
                seg.layout.x_for_index(i)
            } + seg.offset;
            for _ in 0..i {
                expected += px(0.1);
            }
            let mut word = px(0.);
            for c in p.text[..i].chars() {
                if word_separator(c) {
                    word += px(0.3);
                }
            }
            assert_eq!(actual, expected + word);
        }
    }
}

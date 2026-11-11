//! Координаты набранного сегмента: один проход по глифам и межсловным
//! добавкам. Трекинг кусков сверх общего прибавляется к положению глифа
//! точной суммой (f64) с ОДНИМ округлением до f32 на знак, поэтому проход
//! линеен по длине сегмента: прежняя пошаговая свёртка f32 повторяла все
//! предыдущие добавки для каждого знака и на длинном абзаце была квадратичной.

use super::super::*;
use gpui::{Pixels, px};

/// Положение знака с трекингом: точная сумма округляется один раз.
///
/// `tracking` — точная сумма добавок f32 до этого знака. Сложение f64 точно,
/// пока величины укладываются в 53 бита мантиссы; результат — ближайшее f32
/// к точной сумме (а не накопленная ошибка каждого шага свёртки).
fn tracked(base: f32, tracking: f64) -> f32 {
    if tracking == 0.0 {
        return base;
    }
    (f64::from(base) + tracking) as f32
}

impl Paragraph {
    pub(in crate::text::paragraph) fn segment_positions(&self, seg: &Seg) -> Vec<Pixels> {
        self.positions_with(seg, |_| None)
    }

    /// Положения знаков сегмента; `origin(run)` — начало части, к которой
    /// принадлежит прогон глифов `run`, если сегмент набран частями
    /// (`measure::chunked`: положения глифов там считаются от начала части).
    pub(in crate::text::paragraph) fn positions_with(
        &self,
        seg: &Seg,
        mut origin: impl FnMut(usize) -> Option<Pixels>,
    ) -> Vec<Pixels> {
        let len = seg.end - seg.start;
        crate::work::add!(TextPositionBuildBytes, len + 1);
        let mut positions = vec![seg.layout.width; len + 1];
        // x_for_index берёт ПЕРВЫЙ глиф с индексом >= байта, даже когда
        // индексы повторяются или убывают в визуальном порядке.
        let mut filled = 0;
        for (r, run) in seg.layout.runs.iter().enumerate() {
            let at = origin(r);
            for glyph in &run.glyphs {
                let end = glyph.index.min(len);
                if filled <= end {
                    let x = at.map_or(glyph.position.x, |at| at + glyph.position.x);
                    positions[filled..=end].fill(x);
                    filled = end + 1;
                }
            }
        }
        crate::work::add!(
            TextPositionSteps,
            seg.layout
                .runs
                .iter()
                .map(|r| r.glyphs.len())
                .sum::<usize>()
        );
        positions[0] = px(0.);
        positions[len] = seg.layout.width;
        if self.joined_spacing() {
            let prefix = self.spacing_prefix(seg.start, seg.end);
            let mut unit = 0;
            for (at, value) in positions.iter_mut().enumerate() {
                while unit + 1 < prefix.len() && prefix[unit + 1].0 <= seg.start + at {
                    unit += 1;
                }
                *value = (*value + seg.offset) + prefix[unit].1;
            }
            crate::work::add!(TextPositionSteps, len + 1);
            return positions;
        }
        let mut tracking = 0f64;
        let mut words = px(0.);
        for (off, ch) in self.text[seg.start..seg.end].char_indices() {
            let at = seg.start + off;
            let base = f32::from(positions[off] + seg.offset);
            positions[off] = px(tracked(base, tracking)) + words;
            if let Some(i) = self.prepared().letters.first(at) {
                tracking += f64::from(f32::from(self.letter_spans[i].1 - self.letter_spacing));
            }
            if word_separator(ch) {
                words += self
                    .prepared()
                    .words
                    .first(at)
                    .map_or(self.word_spacing, |i| self.word_spans[i].1);
            }
        }
        crate::work::add!(
            TextPositionSteps,
            self.text[seg.start..seg.end].chars().count()
        );
        positions[len] = px(tracked(f32::from(seg.layout.width + seg.offset), tracking)) + words;
        positions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Положение — ближайшее f32 к ТОЧНОЙ сумме: и там, где пошаговая свёртка
    /// точна (результаты совпадают), и там, где она копила ошибку округления
    /// на каждом шаге (переход через двоичный порядок, недвоичные добавки).
    #[test]
    fn tracking_is_rounded_once_from_the_exact_sum() {
        for deltas in [
            vec![0.25; 128],
            vec![0.1; 128],
            vec![16_777_216., 1., -16_777_216.],
            vec![f32::MIN_POSITIVE, -1.0],
        ] {
            for base in [0.0, -0.0, 15.125, 2047.9, 16_383.999, 16_777_216., -100.1] {
                let mut sum = 0f64;
                for end in 0..=deltas.len() {
                    let exact: f64 =
                        f64::from(base) + deltas[..end].iter().map(|d| f64::from(*d)).sum::<f64>();
                    let expected = if end == 0 { base } else { exact as f32 };
                    assert_eq!(
                        tracked(base, sum).to_bits(),
                        expected.to_bits(),
                        "{base} {end}"
                    );
                    if end < deltas.len() {
                        sum += f64::from(deltas[end]);
                    }
                }
                // Где свёртка точна, результат тот же, что у неё.
                let folded = deltas.iter().fold(base, |x, d| x + d);
                if f64::from(folded) == f64::from(base) + sum {
                    assert_eq!(tracked(base, sum), folded);
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
            let glyph = if i == 0 {
                px(0.)
            } else if i == 5 {
                seg.layout.width
            } else {
                seg.layout.x_for_index(i)
            } + seg.offset;
            let tracking = f64::from(0.1f32) * i as f64;
            let mut expected = px(tracked(f32::from(glyph), tracking));
            let mut word = px(0.);
            for c in p.text[..i].chars() {
                if word_separator(c) {
                    word += px(0.3);
                }
            }
            expected += word;
            assert_eq!(actual, expected);
        }
    }

    /// Длинный сегмент строится за один проход: число шагов линейно по длине
    /// (прежняя свёртка повторяла все предыдущие добавки на каждом знаке).
    #[cfg(feature = "work-counters")]
    #[test]
    fn long_tracked_segment_is_built_in_linear_steps() {
        use gpui::{LineLayout, ShapedGlyph, ShapedRun, point};
        let n = 4096;
        let text: String = "ab".repeat(n / 2);
        let p = Paragraph::new(
            text.clone().into(),
            vec![],
            px(16.),
            px(20.),
            Align::Justify,
            Wrap::default(),
        )
        // Выключка: трекинг прибавляется по знакам, а не общим путём.
        .letter_spans(vec![(0..n, px(0.25))]);
        let seg = Seg {
            start: 0,
            end: n,
            offset: px(0.),
            positions: Default::default(),
            layout: std::sync::Arc::new(LineLayout {
                width: px(n as f32 * 1234.567),
                len: n,
                runs: vec![ShapedRun {
                    font_id: gpui::FontId(0),
                    font_size: px(16.),
                    glyphs: (0..n)
                        .map(|i| ShapedGlyph {
                            id: gpui::GlyphId(0),
                            index: i,
                            position: point(px(i as f32 * 1234.567), px(0.)),
                            is_emoji: false,
                        })
                        .collect(),
                }],
                ..Default::default()
            }),
        };
        let session = crate::work::Session::begin();
        let _ = p.segment_positions(&seg);
        let steps: u64 = session
            .finish()
            .0
            .iter()
            .map(|phase| phase[crate::work::Counter::TextPositionSteps as usize])
            .sum();
        assert!(steps <= 2 * n as u64 + 2, "{steps} steps for {n} chars");
    }
}

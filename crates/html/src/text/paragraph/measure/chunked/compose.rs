//! Сборка набора куска из частей: глифы частей из памяти копируются (со
//! сдвигом индексов), остальные части набираются без памяти строк.

use super::*;

impl Paragraph {
    /// Набор куска `start..end` с частями до `ends` и ключами `keys`; места
    /// частей в нём — в `pieces`.
    pub(super) fn compose(
        &self,
        start: usize,
        end: usize,
        ends: &[usize],
        keys: &[u64],
        pieces: &mut [Piece; PIECES],
        text: &WindowTextSystem,
    ) -> LineLayout {
        let mut found: [Option<LinePiece>; PIECES] = std::array::from_fn(|_| None);
        let (mut from, mut capacity, mut widest) = (start, 0, 0);
        for ((slot, &key), &to) in found.iter_mut().zip(keys).zip(ends) {
            *slot = text.line_piece(key);
            let own = self.run_indices(&(from..to)).len();
            if slot.is_none() {
                widest = widest.max(own);
            }
            capacity += slot.as_ref().map_or(own, |c| c.runs.len());
            from = to;
        }
        // Прогоны шрифтов набираемых частей: один блок на всю сборку.
        let mut font_runs = Vec::with_capacity(widest);
        // Ёмкость — степень двойки, как у списка, растущего удвоением.
        let mut runs = Vec::with_capacity(capacity.next_power_of_two().max(4));
        let (mut from, mut width) = (start, px(0.));
        for (i, &to) in ends.iter().enumerate() {
            let (base, first) = (from - start, runs.len());
            let (advance, ascent, descent) = match &found[i] {
                Some(c) => copy_into(c, base, &mut runs),
                None => {
                    let at = self.run_indices(&(from..to));
                    let skip = at.clone().next().map_or(0, |i| {
                        from - (self.prepared().run_ends[i] - self.runs[i].len)
                    });
                    text.shape_into(
                        &self.text[from..to],
                        self.font_size,
                        &self.runs[at],
                        skip,
                        tracking(self),
                        base,
                        &mut runs,
                        &mut font_runs,
                    )
                }
            };
            pieces[i] = Piece {
                runs: (first as u32, runs.len() as u32),
                base: base as u32,
                width: advance,
                ascent,
                descent,
            };
            width += advance;
            from = to;
        }
        let pieces = &pieces[..ends.len()];
        LineLayout {
            font_size: self.font_size,
            width,
            ascent: pieces.iter().map(|p| p.ascent).fold(px(0.), Pixels::max),
            descent: pieces.iter().map(|p| p.descent).fold(px(0.), Pixels::max),
            runs,
            len: end - start,
        }
    }
}

/// Глифы части `c` — в набор куска, где она начинается байтом `base`;
/// возвращает её ширину, подъём и спуск.
///
/// DirectWrite отдаёт пробел в конце набранного текста отдельным прогоном
/// глифов того же шрифта; в наборе целиком он идёт в прогоне слова перед ним.
/// Копия сливает такие прогоны (тот же шрифт и кегль, слева направо подряд)
/// в один, как в наборе целиком.
fn copy_into(c: &LinePiece, base: usize, out: &mut Vec<ShapedRun>) -> (Pixels, Pixels, Pixels) {
    let runs = &c.layout.runs[c.runs.start as usize..c.runs.end as usize];
    let index = |i: usize| i - c.base as usize + base;
    let mut i = 0;
    while i < runs.len() {
        let mut j = i + 1;
        while j < runs.len() && continues(&runs[j - 1], &runs[j]) {
            j += 1;
        }
        let mut glyphs = Vec::with_capacity(runs[i..j].iter().map(|r| r.glyphs.len()).sum());
        for run in &runs[i..j] {
            glyphs.extend(run.glyphs.iter().map(|g| ShapedGlyph {
                id: g.id,
                position: g.position,
                index: index(g.index),
                is_emoji: g.is_emoji,
            }));
        }
        // По одному: список растёт удвоением, как у набора.
        out.push(ShapedRun {
            font_id: runs[i].font_id,
            font_size: runs[i].font_size,
            glyphs,
        });
        i = j;
    }
    (c.width, c.ascent, c.descent)
}

/// Прогон `b` продолжает `a`: тот же шрифт и кегль, глифы дальше по тексту и
/// по строке.
fn continues(a: &ShapedRun, b: &ShapedRun) -> bool {
    a.font_id == b.font_id
        && a.font_size == b.font_size
        && match (a.glyphs.last(), b.glyphs.first()) {
            (Some(x), Some(y)) => y.index > x.index && y.position.x >= x.position.x,
            _ => false,
        }
}

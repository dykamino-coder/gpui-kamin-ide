//! Сборка набора куска из частей: части из памяти копируются со сдвигом,
//! остальные набираются без памяти строк текстовой системы.

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
        let (mut from, mut capacity) = (start, 0);
        for ((slot, &key), &to) in found.iter_mut().zip(keys).zip(ends) {
            *slot = text.line_piece(key);
            capacity += slot
                .as_ref()
                .map_or_else(|| self.run_indices(&(from..to)).len(), |c| c.runs.len());
            from = to;
        }
        // Ёмкость — степень двойки, как у списка, растущего удвоением.
        let mut runs = Vec::with_capacity(capacity.next_power_of_two().max(4));
        let (mut from, mut origin) = (start, px(0.));
        for (i, &to) in ends.iter().enumerate() {
            let (base, first) = (from - start, runs.len());
            let (width, ascent, descent) = match &found[i] {
                Some(c) => copy_into(c, base, origin, &mut runs),
                None => {
                    let at = self.run_indices(&(from..to));
                    let skip = at.clone().next().map_or(0, |i| {
                        from - (self.prepared().run_ends[i] - self.runs[i].len)
                    });
                    let metrics = text.shape_into(
                        &self.text[from..to],
                        self.font_size,
                        &self.runs[at],
                        skip,
                        tracking(self),
                        base,
                        &mut runs,
                    );
                    for glyph in runs[first..].iter_mut().flat_map(|r| &mut r.glyphs) {
                        glyph.position.x += origin;
                    }
                    metrics
                }
            };
            pieces[i] = Piece {
                runs: (first as u32, runs.len() as u32),
                base: base as u32,
                origin,
                width,
                ascent,
                descent,
            };
            origin += width;
            from = to;
        }
        let pieces = &pieces[..ends.len()];
        LineLayout {
            font_size: self.font_size,
            width: origin,
            ascent: pieces.iter().map(|p| p.ascent).fold(px(0.), Pixels::max),
            descent: pieces.iter().map(|p| p.descent).fold(px(0.), Pixels::max),
            runs,
            len: end - start,
        }
    }
}

/// Глифы части `c` — в набор куска, где она начинается байтом `base` и
/// пером `origin`; возвращает её ширину, подъём и спуск.
fn copy_into(
    c: &LinePiece,
    base: usize,
    origin: Pixels,
    out: &mut Vec<ShapedRun>,
) -> (Pixels, Pixels, Pixels) {
    let from = c.runs.start as usize..c.runs.end as usize;
    let (shift, index) = (origin - c.origin, |i: usize| i - c.base as usize + base);
    // По одному: список растёт удвоением, как у набора.
    for run in &c.layout.runs[from] {
        out.push(ShapedRun {
            font_id: run.font_id,
            font_size: run.font_size,
            glyphs: run
                .glyphs
                .iter()
                .map(|g| ShapedGlyph {
                    id: g.id,
                    position: gpui::point(g.position.x + shift, g.position.y),
                    index: index(g.index),
                    is_emoji: g.is_emoji,
                })
                .collect(),
        });
    }
    (c.width, c.ascent, c.descent)
}

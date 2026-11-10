//! Сборка набора куска из частей: общий набор того же текста, копии частей
//! из памяти и набор новых частей без памяти строк текстовой системы.

use super::*;

impl Paragraph {
    /// Набор куска, если все его части лежат подряд в одном наборе того же
    /// куска (тот же текст, измеренный другим замером).
    pub(super) fn same_layout(
        &self,
        start: usize,
        end: usize,
        ends: &[usize],
        found: &[Option<Chunk>],
    ) -> Option<Arc<LineLayout>> {
        let first = found.first()?.as_ref()?;
        let (mut from, mut run) = (start, 0);
        for (c, &to) in found.iter().zip(ends) {
            let c = c.as_ref()?;
            if !Arc::ptr_eq(&c.layout, &first.layout)
                || c.runs.start != run
                || c.base as usize != from - start
            {
                return None;
            }
            (from, run) = (to, c.runs.end);
        }
        (run as usize == first.layout.runs.len() && first.layout.len == end - start)
            .then(|| first.layout.clone())
    }

    /// Новый набор куска: части из памяти копируются, остальные набираются.
    pub(super) fn compose(
        &self,
        start: usize,
        end: usize,
        ends: &[usize],
        found: &[Option<Chunk>],
        pieces: &mut [Piece; PIECES],
        window: &mut Window,
    ) -> Arc<LineLayout> {
        let text = window.text_system().with_ligature_breaking(false);
        let (mut from, mut capacity) = (start, 0);
        for (c, &to) in found.iter().zip(ends) {
            capacity += c
                .as_ref()
                .map_or_else(|| self.run_indices(&(from..to)).len(), |c| c.runs.len());
            from = to;
        }
        let mut runs = Vec::with_capacity(capacity);
        let (mut from, mut origin) = (start, px(0.));
        for (i, (c, &to)) in found.iter().zip(ends).enumerate() {
            let base = from - start;
            let (width, ascent, descent) = match c {
                Some(c) => c.copy_into(base, &mut runs),
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
                    )
                }
            };
            pieces[i] = Piece {
                base: base as u32,
                runs_end: runs.len() as u32,
                width,
                ascent,
                descent,
                origin,
            };
            origin += width;
            from = to;
        }
        let pieces = &pieces[..found.len()];
        Arc::new(LineLayout {
            font_size: self.font_size,
            width: origin,
            ascent: pieces.iter().map(|p| p.ascent).fold(px(0.), Pixels::max),
            descent: pieces.iter().map(|p| p.descent).fold(px(0.), Pixels::max),
            runs,
            len: end - start,
        })
    }
}

impl Chunk {
    /// Часть общего набора с началом `origin`.
    pub(super) fn piece(&self, origin: Pixels) -> Piece {
        Piece {
            base: self.base,
            runs_end: self.runs.end,
            width: self.width,
            ascent: self.ascent,
            descent: self.descent,
            origin,
        }
    }

    /// Глифы части — в набор куска, где она начинается со смещения `base`.
    fn copy_into(&self, base: usize, out: &mut Vec<ShapedRun>) -> (Pixels, Pixels, Pixels) {
        let from = self.runs.start as usize..self.runs.end as usize;
        let shift = |index: usize| index - self.base as usize + base;
        out.extend(self.layout.runs[from].iter().map(|run| {
            ShapedRun {
                font_id: run.font_id,
                font_size: run.font_size,
                glyphs: run
                    .glyphs
                    .iter()
                    .map(|g| ShapedGlyph {
                        id: g.id,
                        position: g.position,
                        index: shift(g.index),
                        is_emoji: g.is_emoji,
                    })
                    .collect(),
            }
        }));
        (self.width, self.ascent, self.descent)
    }
}

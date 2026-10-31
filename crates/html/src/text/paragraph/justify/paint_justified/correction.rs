//! Вертикальное совмещение отдельно набранных слов и промежутков строки.

use crate::text::paragraph::Paragraph;
use gpui::{Pixels, px};

impl Paragraph {
    /// Vertical correction of a separately shaped piece of the line: its own
    /// baseline is moved onto the line's, or, inside a top/bottom aligned
    /// inline box, onto that box's edge (CSS 2.1 section 10.8.1). Words and
    /// the inter-word gaps of the same box share it, so their backgrounds
    /// form one band.
    pub(super) fn piece_fix(
        &self,
        range: &std::ops::Range<usize>,
        line_base: Option<f32>,
        pad: (f32, f32),
    ) -> Pixels {
        self.piece_fix_with(range, self.base_of(range), line_base, pad)
    }

    /// `piece_fix` with the caller's baseline (font metric overrides).
    pub(super) fn piece_fix_with(
        &self,
        range: &std::ops::Range<usize>,
        piece_base: Option<f32>,
        line_base: Option<f32>,
        pad: (f32, f32),
    ) -> Pixels {
        let fix = match (line_base, piece_base) {
            (Some(l), Some(w)) => px(l - w),
            _ => px(0.),
        };
        // Кусок у края строки: его строчная коробка (высотой своей
        // `line-height`, глифы по её полулидингу) встаёт верхом на верх
        // строки или низом на низ (§10.8.1). `shaped.paint` центрирует
        // глифы в высоте строки абзаца — разница высот делится пополам.
        match self
            .prepared()
            .edges
            .first(range.start)
            .map(|i| &self.edge_spans[i])
        {
            Some((r, top, h)) => {
                let lh = f32::from(self.line_height);
                let line_top = -pad.0;
                let box_top = if *top { line_top } else { lh + pad.1 - h };
                // Align the subtree as one box, preserving its common
                // baseline (CSS 2.1 section 10.8.1). Centering each word
                // separately misaligns descendants with different fonts.
                let baseline = match (self.base_of(r), piece_base) {
                    (Some(group), Some(word)) => group - word,
                    _ => 0.0,
                };
                px(box_top + (h - lh) / 2.0 + baseline)
            }
            None => fix,
        }
    }
}

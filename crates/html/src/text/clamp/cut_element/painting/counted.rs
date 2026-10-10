//! Counted for painting; split out to keep the owning module within 250 lines.

use super::ClampCut;
use super::ClampEntry;

impl ClampCut {
    pub(crate) fn counted_cut(
        &self,
        mut cut: Option<f32>,
        entries: &[ClampEntry],
        blocks: &[(f32, f32, bool, f32)],
        split: &dyn Fn(&ClampEntry) -> Vec<(f32, f32)>,
    ) -> (Option<f32>, Option<(u32, usize)>, bool) {
        let mut num_para: Option<(u32, usize)> = None;
        let mut by_count = false;
        if let Some(limit) = self.limit.filter(|n| *n > 0) {
            // (верх строки, низ строки, номер абзаца, номер строки в абзаце)
            let mut marks: Vec<(f32, f32, u32, usize)> = vec![];
            for e in entries.iter().filter(|e| e.line > 0.0 && !e.skip_count) {
                let Some(seq) = e.seq else { continue };
                let h = f32::from(e.bounds.size.height);
                if h <= 0.0 {
                    continue;
                }
                for (i, (a, b)) in split(e).into_iter().enumerate() {
                    marks.push((a, b, seq, i + 1));
                }
            }
            marks.sort_by(|a, b| a.0.total_cmp(&b.0));
            match marks.get(limit as usize - 1).copied() {
                Some((_, bottom, seq, k)) if cut.is_none_or(|c| bottom <= c + 0.5) => {
                    // Остаток СВОЕГО абзаца — знак нужен, потолок нет: абзац
                    // укоротит бюджет.
                    let own_rest = entries.iter().any(|e| {
                        e.line > 0.0
                            && e.seq == Some(seq)
                            && f32::from(e.bounds.origin.y) + f32::from(e.bounds.size.height)
                                > bottom + 0.5
                    });
                    // Другое содержимое за точкой: чужой абзац, выходящий за
                    // неё (в том числе несчитаемый), или коробка, начатая после.
                    let follows = entries.iter().any(|e| {
                        let y0 = f32::from(e.bounds.origin.y);
                        let h = f32::from(e.bounds.size.height);
                        h > 0.0
                            && if e.line > 0.0 {
                                e.seq != Some(seq) && y0 + h > bottom + 0.5
                            } else {
                                y0 >= bottom - 0.5
                            }
                    });
                    if own_rest || follows || entries.iter().any(|e| e.clamped.is_some()) {
                        num_para = Some((seq, k));
                    }
                    by_count = true;
                    if follows {
                        // Коробки, содержащие точку, фрагментированы в ней и
                        // уносят свои нижние рамку и паддинг (§5.3); коробка с
                        // заданной высотой не фрагментируется — видна целиком.
                        let mut add = 0.0f32;
                        let mut floor = bottom;
                        for (y0, y1, fixed, bp) in blocks {
                            if *y0 < bottom && bottom < *y1 {
                                if *fixed {
                                    floor = floor.max(*y1);
                                } else {
                                    add += *bp;
                                }
                            }
                        }
                        cut = Some((bottom + add).max(floor));
                    }
                }
                // `max-height` теснее N строк — дальше как в авто-режиме.
                Some(_) => {}
                None => {
                    if self.max_h.is_none() {
                        // Строк меньше предела — среза нет.
                        cut = None;
                    }
                }
            }
        }
        (cut, num_para, by_count)
    }
}
